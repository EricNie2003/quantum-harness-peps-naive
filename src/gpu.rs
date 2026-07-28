//! Optional exact CUDA contraction backend.
//!
//! Rust remains the source of truth for the explicit Sec. VI tensors. This
//! module compiles the already-validated row operator into a C-compatible
//! descriptor and sends that descriptor to CUDA for expansion, sort, and
//! exact reduction.

use crate::{CompiledRowOperator, ContractionResult, LayerMetric, SiteTensorC};
use std::error::Error;
use std::fmt;
use std::mem::{align_of, size_of};
use std::time::Duration;

const SCHEME_COMPACT64: u32 = 1;
const SCHEME_WIDE128: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuScheme {
    Auto,
    Compact64,
    Wide128,
}

impl GpuScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Compact64 => "compact64",
            Self::Wide128 => "wide128",
        }
    }

    pub fn parse(value: &str) -> Result<Self, GpuError> {
        match value {
            "auto" => Ok(Self::Auto),
            "compact64" => Ok(Self::Compact64),
            "wide128" => Ok(Self::Wide128),
            _ => Err(GpuError::InvalidArgument(format!(
                "unknown GPU scheme {value:?}; expected auto, compact64, or wide128"
            ))),
        }
    }

    fn resolve(self, n: usize) -> Result<Self, GpuError> {
        if n > 42 {
            return Err(GpuError::InvalidArgument(
                "wide128 packed boundaries support N <= 42".to_owned(),
            ));
        }
        match self {
            Self::Auto if n <= 20 => Ok(Self::Compact64),
            Self::Auto => Ok(Self::Wide128),
            Self::Compact64 if n > 20 => Err(GpuError::InvalidArgument(
                "compact64 is exact only for N <= 20".to_owned(),
            )),
            scheme => Ok(scheme),
        }
    }

    fn ffi_code(self) -> u32 {
        match self {
            Self::Compact64 => SCHEME_COMPACT64,
            Self::Wide128 => SCHEME_WIDE128,
            Self::Auto => unreachable!("GPU scheme must be resolved before FFI"),
        }
    }
}

impl fmt::Display for GpuScheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GpuOptions {
    pub device_id: u32,
    pub scheme: GpuScheme,
    pub memory_limit_percent: u32,
}

impl Default for GpuOptions {
    fn default() -> Self {
        Self {
            device_id: 0,
            scheme: GpuScheme::Auto,
            memory_limit_percent: 85,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GpuDeviceInfo {
    pub device_id: u32,
    pub name: String,
    pub compute_major: u32,
    pub compute_minor: u32,
    pub cuda_driver_version: u32,
    pub cuda_runtime_version: u32,
    pub multiprocessor_count: u32,
    pub total_global_memory: u64,
}

#[derive(Clone, Debug)]
pub struct GpuLayerMetric {
    pub row: usize,
    pub input_states: usize,
    pub row_operator_candidates: u128,
    pub row_operator_matched: u128,
    pub completed_row_terms: u128,
    pub output_states: usize,
    pub output_weight: u128,
    pub count_scan: Duration,
    pub expansion: Duration,
    pub sort: Duration,
    pub run_length: Duration,
    pub reduction: Duration,
    pub metric: Duration,
    pub peak_device_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct GpuContractionResult {
    pub n: usize,
    pub count: u128,
    pub scheme: GpuScheme,
    pub device: GpuDeviceInfo,
    pub host_elapsed: Duration,
    pub gpu_elapsed: Duration,
    pub peak_host_rss_bytes: u64,
    pub peak_device_bytes: u64,
    pub peak_states: usize,
    pub tensor_entries_examined: u128,
    pub tensor_entries_matched: u128,
    pub row_operator_candidates: u128,
    pub row_operator_matched: u128,
    pub layers: Vec<GpuLayerMetric>,
}

impl GpuContractionResult {
    /// Convert common fields to the CPU result shape for validation helpers.
    /// GPU phase timings and device allocation metrics remain available on
    /// this value and are intentionally not hidden in the CPU RSS field.
    pub fn common_result(&self) -> ContractionResult {
        ContractionResult {
            n: self.n,
            count: self.count,
            elapsed: self.host_elapsed,
            peak_states: self.peak_states,
            tensor_entries_examined: self.tensor_entries_examined,
            tensor_entries_matched: self.tensor_entries_matched,
            row_operator_candidates: self.row_operator_candidates,
            row_operator_matched: self.row_operator_matched,
            peak_rss_bytes: self.peak_host_rss_bytes,
            layers: self
                .layers
                .iter()
                .map(|layer| LayerMetric {
                    row: layer.row,
                    input_states: layer.input_states,
                    tensor_entries_examined: 0,
                    tensor_entries_matched: 0,
                    row_operator_candidates: layer.row_operator_candidates,
                    row_operator_matched: layer.row_operator_matched,
                    completed_row_terms: layer.completed_row_terms,
                    output_states: layer.output_states,
                    output_weight: layer.output_weight,
                    elapsed: layer.count_scan
                        + layer.expansion
                        + layer.sort
                        + layer.run_length
                        + layer.reduction
                        + layer.metric,
                    peak_rss_bytes: 0,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuError {
    InvalidArgument(String),
    Backend(String),
    Abi(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "invalid GPU argument: {message}"),
            Self::Backend(message) => write!(formatter, "CUDA backend error: {message}"),
            Self::Abi(message) => write!(formatter, "CUDA ABI error: {message}"),
        }
    }
}

impl Error for GpuError {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct TransitionDescriptorFfi {
    column_in: u32,
    column_out: u32,
    row_in: u32,
    row_out: u32,
    diag_dr_in: u32,
    diag_dr_out: u32,
    diag_dl_in: u32,
    diag_dl_out: u32,
    value_lo: u64,
    value_hi: u64,
    tensor_entries_examined: u64,
    tensor_entries_matched: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct OptionsFfi {
    device_id: u32,
    scheme: u32,
    memory_limit_percent: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct LayerMetricFfi {
    row: u64,
    input_states: u64,
    row_operator_candidates: u64,
    row_operator_matched: u64,
    completed_row_terms: u64,
    output_states: u64,
    output_weight_lo: u64,
    output_weight_hi: u64,
    count_scan_ns: u64,
    expansion_ns: u64,
    sort_ns: u64,
    run_length_ns: u64,
    reduction_ns: u64,
    metric_ns: u64,
    peak_device_bytes: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct RunResultFfi {
    n: u32,
    scheme: u32,
    device_id: u32,
    compute_major: u32,
    compute_minor: u32,
    cuda_driver_version: u32,
    cuda_runtime_version: u32,
    reserved: u32,
    count_lo: u64,
    count_hi: u64,
    host_elapsed_ns: u64,
    gpu_elapsed_ns: u64,
    peak_device_bytes: u64,
    peak_states: u64,
    tensor_entries_examined: u64,
    tensor_entries_matched: u64,
    row_operator_candidates: u64,
    row_operator_matched: u64,
    layer_count: u64,
    device_name: [u8; 128],
    error: [u8; 512],
}

impl Default for RunResultFfi {
    fn default() -> Self {
        Self {
            n: 0,
            scheme: 0,
            device_id: 0,
            compute_major: 0,
            compute_minor: 0,
            cuda_driver_version: 0,
            cuda_runtime_version: 0,
            reserved: 0,
            count_lo: 0,
            count_hi: 0,
            host_elapsed_ns: 0,
            gpu_elapsed_ns: 0,
            peak_device_bytes: 0,
            peak_states: 0,
            tensor_entries_examined: 0,
            tensor_entries_matched: 0,
            row_operator_candidates: 0,
            row_operator_matched: 0,
            layer_count: 0,
            device_name: [0; 128],
            error: [0; 512],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct DeviceInfoFfi {
    device_id: u32,
    compute_major: u32,
    compute_minor: u32,
    cuda_driver_version: u32,
    cuda_runtime_version: u32,
    multiprocessor_count: u32,
    reserved0: u32,
    reserved1: u32,
    total_global_memory: u64,
    device_name: [u8; 128],
    error: [u8; 512],
}

impl Default for DeviceInfoFfi {
    fn default() -> Self {
        Self {
            device_id: 0,
            compute_major: 0,
            compute_minor: 0,
            cuda_driver_version: 0,
            cuda_runtime_version: 0,
            multiprocessor_count: 0,
            reserved0: 0,
            reserved1: 0,
            total_global_memory: 0,
            device_name: [0; 128],
            error: [0; 512],
        }
    }
}

unsafe extern "C" {
    fn nq_gpu_probe(device_id: u32, info: *mut DeviceInfoFfi) -> i32;
    fn nq_gpu_self_test(device_id: u32, error: *mut u8, error_capacity: usize) -> i32;
    fn nq_gpu_contract(
        n: u32,
        descriptor: *const TransitionDescriptorFfi,
        options: *const OptionsFfi,
        result: *mut RunResultFfi,
        layers: *mut LayerMetricFfi,
        layer_capacity: usize,
    ) -> i32;
}

fn limbs(value: u128) -> (u64, u64) {
    (value as u64, (value >> 64) as u64)
}

fn from_limbs(low: u64, high: u64) -> u128 {
    u128::from(low) | (u128::from(high) << 64)
}

fn ffi_string(bytes: &[u8]) -> String {
    let length = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..length]).into_owned()
}

fn usize_field(value: u64, label: &str) -> Result<usize, GpuError> {
    usize::try_from(value).map_err(|_| GpuError::Abi(format!("{label}={value} does not fit usize")))
}

fn compile_descriptor() -> Result<TransitionDescriptorFfi, GpuError> {
    let tensor = SiteTensorC::sec_vi();
    if tensor.entries().len() != 17 {
        return Err(GpuError::Abi(format!(
            "explicit C has {} entries instead of 17",
            tensor.entries().len()
        )));
    }
    let operator = CompiledRowOperator::compile(&tensor).map_err(GpuError::Abi)?;
    let occupied = operator.occupied;
    let (value_lo, value_hi) = limbs(occupied.value);
    Ok(TransitionDescriptorFfi {
        column_in: u32::from(occupied.legs.column_in),
        column_out: u32::from(occupied.legs.column_out),
        row_in: u32::from(occupied.legs.row_in),
        row_out: u32::from(occupied.legs.row_out),
        diag_dr_in: u32::from(occupied.legs.diag_dr_in),
        diag_dr_out: u32::from(occupied.legs.diag_dr_out),
        diag_dl_in: u32::from(occupied.legs.diag_dl_in),
        diag_dl_out: u32::from(occupied.legs.diag_dl_out),
        value_lo,
        value_hi,
        tensor_entries_examined: 17,
        tensor_entries_matched: 17,
    })
}

pub fn probe_device(device_id: u32) -> Result<GpuDeviceInfo, GpuError> {
    validate_abi()?;
    let mut ffi = DeviceInfoFfi::default();
    // SAFETY: `ffi` is a valid writable C-compatible structure for the full
    // duration of the call.
    let status = unsafe { nq_gpu_probe(device_id, &mut ffi) };
    if status != 0 {
        return Err(GpuError::Backend(ffi_string(&ffi.error)));
    }
    Ok(GpuDeviceInfo {
        device_id: ffi.device_id,
        name: ffi_string(&ffi.device_name),
        compute_major: ffi.compute_major,
        compute_minor: ffi.compute_minor,
        cuda_driver_version: ffi.cuda_driver_version,
        cuda_runtime_version: ffi.cuda_runtime_version,
        multiprocessor_count: ffi.multiprocessor_count,
        total_global_memory: ffi.total_global_memory,
    })
}

pub fn run_device_self_test(device_id: u32) -> Result<(), GpuError> {
    validate_abi()?;
    let mut error = [0_u8; 512];
    // SAFETY: the error buffer is writable for exactly the supplied capacity.
    let status = unsafe { nq_gpu_self_test(device_id, error.as_mut_ptr(), error.len()) };
    if status != 0 {
        return Err(GpuError::Backend(ffi_string(&error)));
    }
    Ok(())
}

pub fn contract_rows_gpu(n: usize, options: GpuOptions) -> Result<GpuContractionResult, GpuError> {
    validate_abi()?;
    if !(1..=95).contains(&options.memory_limit_percent) {
        return Err(GpuError::InvalidArgument(
            "memory_limit_percent must be in 1..=95".to_owned(),
        ));
    }
    let scheme = options.scheme.resolve(n)?;
    let n_u32 = u32::try_from(n)
        .map_err(|_| GpuError::InvalidArgument(format!("N={n} does not fit u32")))?;
    let descriptor = compile_descriptor()?;
    let device = probe_device(options.device_id)?;
    let ffi_options = OptionsFfi {
        device_id: options.device_id,
        scheme: scheme.ffi_code(),
        memory_limit_percent: options.memory_limit_percent,
        reserved: 0,
    };
    let mut ffi_result = RunResultFfi::default();
    let mut ffi_layers = vec![LayerMetricFfi::default(); n];
    // SAFETY: all pointers refer to initialized C-compatible objects; the
    // layer pointer has the capacity reported in `layer_capacity` and remains
    // alive for the duration of the call.
    let status = unsafe {
        nq_gpu_contract(
            n_u32,
            &descriptor,
            &ffi_options,
            &mut ffi_result,
            ffi_layers.as_mut_ptr(),
            ffi_layers.len(),
        )
    };
    if status != 0 {
        return Err(GpuError::Backend(ffi_string(&ffi_result.error)));
    }
    if ffi_result.n != n_u32 || ffi_result.scheme != scheme.ffi_code() {
        return Err(GpuError::Abi(
            "CUDA returned mismatched N or arithmetic scheme".to_owned(),
        ));
    }
    if ffi_result.device_id != device.device_id
        || ffi_result.compute_major != device.compute_major
        || ffi_result.compute_minor != device.compute_minor
    {
        return Err(GpuError::Abi(
            "CUDA contraction device does not match the probed device".to_owned(),
        ));
    }
    if ffi_result.layer_count != n as u64 {
        return Err(GpuError::Abi(format!(
            "CUDA returned {} layers for N={n}",
            ffi_result.layer_count
        )));
    }
    let layers = ffi_layers
        .into_iter()
        .map(|layer| {
            Ok(GpuLayerMetric {
                row: usize_field(layer.row, "row")?,
                input_states: usize_field(layer.input_states, "input_states")?,
                row_operator_candidates: u128::from(layer.row_operator_candidates),
                row_operator_matched: u128::from(layer.row_operator_matched),
                completed_row_terms: u128::from(layer.completed_row_terms),
                output_states: usize_field(layer.output_states, "output_states")?,
                output_weight: from_limbs(layer.output_weight_lo, layer.output_weight_hi),
                count_scan: Duration::from_nanos(layer.count_scan_ns),
                expansion: Duration::from_nanos(layer.expansion_ns),
                sort: Duration::from_nanos(layer.sort_ns),
                run_length: Duration::from_nanos(layer.run_length_ns),
                reduction: Duration::from_nanos(layer.reduction_ns),
                metric: Duration::from_nanos(layer.metric_ns),
                peak_device_bytes: layer.peak_device_bytes,
            })
        })
        .collect::<Result<Vec<_>, GpuError>>()?;
    Ok(GpuContractionResult {
        n,
        count: from_limbs(ffi_result.count_lo, ffi_result.count_hi),
        scheme,
        device,
        host_elapsed: Duration::from_nanos(ffi_result.host_elapsed_ns),
        gpu_elapsed: Duration::from_nanos(ffi_result.gpu_elapsed_ns),
        peak_host_rss_bytes: crate::peak_rss_bytes(),
        peak_device_bytes: ffi_result.peak_device_bytes,
        peak_states: usize_field(ffi_result.peak_states, "peak_states")?,
        tensor_entries_examined: u128::from(ffi_result.tensor_entries_examined),
        tensor_entries_matched: u128::from(ffi_result.tensor_entries_matched),
        row_operator_candidates: u128::from(ffi_result.row_operator_candidates),
        row_operator_matched: u128::from(ffi_result.row_operator_matched),
        layers,
    })
}

fn validate_abi() -> Result<(), GpuError> {
    let expected = [
        (
            "TransitionDescriptorFfi",
            size_of::<TransitionDescriptorFfi>(),
            64,
        ),
        ("OptionsFfi", size_of::<OptionsFfi>(), 16),
        ("LayerMetricFfi", size_of::<LayerMetricFfi>(), 120),
        ("RunResultFfi", size_of::<RunResultFfi>(), 760),
        ("DeviceInfoFfi", size_of::<DeviceInfoFfi>(), 680),
    ];
    for (name, actual, required) in expected {
        if actual != required {
            return Err(GpuError::Abi(format!(
                "{name} size is {actual}, expected {required}"
            )));
        }
    }
    if align_of::<RunResultFfi>() != 8 || align_of::<LayerMetricFfi>() != 8 {
        return Err(GpuError::Abi(
            "CUDA FFI result structures require 8-byte alignment".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{contract_rows_sort_reduce, known_count};

    #[test]
    fn ffi_layout_and_tensor_descriptor_are_fail_closed() {
        validate_abi().unwrap();
        let descriptor = compile_descriptor().unwrap();
        assert_eq!(descriptor.tensor_entries_examined, 17);
        assert_eq!(descriptor.tensor_entries_matched, 17);
        assert_eq!(descriptor.column_in, 0);
        assert_eq!(descriptor.column_out, 1);
        assert_eq!(descriptor.row_in, 0);
        assert_eq!(descriptor.row_out, 1);
        assert_eq!(descriptor.diag_dr_in, 0);
        assert_eq!(descriptor.diag_dr_out, 1);
        assert_eq!(descriptor.diag_dl_in, 0);
        assert_eq!(descriptor.diag_dl_out, 1);
        assert_eq!(from_limbs(descriptor.value_lo, descriptor.value_hi), 1);
    }

    #[test]
    fn scheme_resolution_is_exact_and_bounded() {
        assert_eq!(GpuScheme::Auto.resolve(20).unwrap(), GpuScheme::Compact64);
        assert_eq!(GpuScheme::Auto.resolve(21).unwrap(), GpuScheme::Wide128);
        assert_eq!(GpuScheme::Auto.resolve(42).unwrap(), GpuScheme::Wide128);
        assert!(GpuScheme::Auto.resolve(43).is_err());
        assert!(GpuScheme::Compact64.resolve(21).is_err());
        assert!((1_u128..=20).product::<u128>() < (1_u128 << 64));
    }

    #[test]
    fn gpu_schemes_match_every_cpu_layer_through_n10_when_available() {
        if probe_device(0).is_err() {
            eprintln!("skipping CUDA integration test because device 0 is unavailable");
            return;
        }
        run_device_self_test(0).unwrap();
        for n in 0..=10 {
            let cpu = contract_rows_sort_reduce(n).unwrap();
            for scheme in [GpuScheme::Compact64, GpuScheme::Wide128] {
                let gpu = contract_rows_gpu(
                    n,
                    GpuOptions {
                        scheme,
                        ..GpuOptions::default()
                    },
                )
                .unwrap();
                assert_eq!(gpu.count, cpu.count, "count mismatch at N={n}, {scheme}");
                assert_eq!(
                    gpu.count,
                    known_count(n).unwrap(),
                    "known Q mismatch at N={n}"
                );
                assert_eq!(
                    gpu.peak_states, cpu.peak_states,
                    "support mismatch at N={n}"
                );
                assert_eq!(
                    gpu.row_operator_candidates, cpu.row_operator_candidates,
                    "candidate mismatch at N={n}"
                );
                assert_eq!(
                    gpu.row_operator_matched, cpu.row_operator_matched,
                    "matched mismatch at N={n}"
                );
                let gpu_layers = gpu
                    .layers
                    .iter()
                    .map(|layer| {
                        (
                            layer.input_states,
                            layer.completed_row_terms,
                            layer.output_states,
                            layer.output_weight,
                        )
                    })
                    .collect::<Vec<_>>();
                let cpu_layers = cpu
                    .layers
                    .iter()
                    .map(|layer| {
                        (
                            layer.input_states,
                            layer.completed_row_terms,
                            layer.output_states,
                            layer.output_weight,
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(gpu_layers, cpu_layers, "layer mismatch at N={n}, {scheme}");
            }
        }
    }
}
