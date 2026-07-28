use nqueens_peps_naive::gpu::{
    GpuContractionResult, GpuOptions, GpuScheme, contract_rows_gpu, probe_device,
    run_device_self_test,
};
use nqueens_peps_naive::known_count;
use std::env;
use std::process::ExitCode;
use std::time::Duration;

fn usage() -> &'static str {
    "Usage:\n  gpu_sort_reduce probe [--device ID]\n  \
     gpu_sort_reduce self-test [--device ID]\n  \
     gpu_sort_reduce solve N [--device ID] [--scheme auto|compact64|wide128] \
     [--memory-limit-percent P] [--layers]\n  \
     gpu_sort_reduce bench MAX_N [--min N] [--device ID] \
     [--scheme auto|compact64|wide128] [--memory-limit-percent P] \
     [--warmup W] [--repeats R] [--csv]"
}

fn parse_usize(value: Option<&String>, label: &str) -> Result<usize, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn parse_u32(value: Option<&String>, label: &str) -> Result<u32, String> {
    value
        .ok_or_else(|| format!("missing {label}"))?
        .parse()
        .map_err(|_| format!("invalid {label}"))
}

fn csv_text(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn version_text(version: u32) -> String {
    format!("{}.{}", version / 1000, (version % 1000) / 10)
}

fn parse_common(
    args: &[String],
    allow_layers: bool,
    allow_benchmark: bool,
) -> Result<(GpuOptions, bool, usize, usize, bool), String> {
    let mut options = GpuOptions::default();
    let mut layers = false;
    let mut warmup = 2_usize;
    let mut repeats = 9_usize;
    let mut csv = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--device" => {
                options.device_id = parse_u32(args.get(index + 1), "--device value")?;
                index += 2;
            }
            "--scheme" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing --scheme value".to_owned())?;
                options.scheme = GpuScheme::parse(value).map_err(|error| error.to_string())?;
                index += 2;
            }
            "--memory-limit-percent" => {
                options.memory_limit_percent =
                    parse_u32(args.get(index + 1), "--memory-limit-percent value")?;
                index += 2;
            }
            "--layers" if allow_layers => {
                layers = true;
                index += 1;
            }
            "--warmup" if allow_benchmark => {
                warmup = parse_usize(args.get(index + 1), "--warmup value")?;
                index += 2;
            }
            "--repeats" if allow_benchmark => {
                repeats = parse_usize(args.get(index + 1), "--repeats value")?;
                index += 2;
            }
            "--csv" if allow_benchmark => {
                csv = true;
                index += 1;
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    if !(1..=95).contains(&options.memory_limit_percent) {
        return Err("--memory-limit-percent must be in 1..=95".to_owned());
    }
    if repeats == 0 {
        return Err("--repeats must be positive".to_owned());
    }
    Ok((options, layers, warmup, repeats, csv))
}

fn probe(args: &[String]) -> Result<(), String> {
    let (options, _, _, _, _) = parse_common(args, false, false)?;
    let device = probe_device(options.device_id).map_err(|error| error.to_string())?;
    println!(
        "device_id={} name={} compute_capability={}.{} global_memory_bytes={} multiprocessors={} cuda_driver={} cuda_runtime={}",
        device.device_id,
        device.name,
        device.compute_major,
        device.compute_minor,
        device.total_global_memory,
        device.multiprocessor_count,
        version_text(device.cuda_driver_version),
        version_text(device.cuda_runtime_version),
    );
    Ok(())
}

fn self_test(args: &[String]) -> Result<(), String> {
    let (options, _, _, _, _) = parse_common(args, false, false)?;
    let device = probe_device(options.device_id).map_err(|error| error.to_string())?;
    run_device_self_test(options.device_id).map_err(|error| error.to_string())?;
    println!(
        "GPU self-test passed: device={} cc={}.{} wide_sort=true run_length=true compact_overflow=true wide_carry=true wide_overflow=true",
        device.name, device.compute_major, device.compute_minor
    );
    Ok(())
}

fn solve(args: &[String]) -> Result<(), String> {
    let n = parse_usize(args.first(), "N")?;
    let (options, show_layers, _, _, _) = parse_common(&args[1..], true, false)?;
    let result = contract_rows_gpu(n, options).map_err(|error| error.to_string())?;
    let expected = known_count(n);
    let verified = expected == Some(result.count);
    println!(
        "backend=gpu scheme={} device={} cc={}.{} N={} Q(N)={} host_elapsed_s={:.9} gpu_elapsed_s={:.9} peak_host_rss_bytes={} peak_device_bytes={} peak_states={} tensor_entries_examined={} tensor_entries_matched={} row_operator_candidates={} row_operator_matched={} verified={}",
        result.scheme,
        result.device.name,
        result.device.compute_major,
        result.device.compute_minor,
        n,
        result.count,
        result.host_elapsed.as_secs_f64(),
        result.gpu_elapsed.as_secs_f64(),
        result.peak_host_rss_bytes,
        result.peak_device_bytes,
        result.peak_states,
        result.tensor_entries_examined,
        result.tensor_entries_matched,
        result.row_operator_candidates,
        result.row_operator_matched,
        verified,
    );
    if show_layers {
        println!(
            "row,input_states,row_operator_candidates,row_operator_matched,completed_row_terms,output_states,output_weight,count_scan_s,expansion_s,sort_s,run_length_s,reduction_s,metric_s,peak_device_bytes"
        );
        for layer in &result.layers {
            println!(
                "{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{}",
                layer.row + 1,
                layer.input_states,
                layer.row_operator_candidates,
                layer.row_operator_matched,
                layer.completed_row_terms,
                layer.output_states,
                layer.output_weight,
                layer.count_scan.as_secs_f64(),
                layer.expansion.as_secs_f64(),
                layer.sort.as_secs_f64(),
                layer.run_length.as_secs_f64(),
                layer.reduction.as_secs_f64(),
                layer.metric.as_secs_f64(),
                layer.peak_device_bytes,
            );
        }
    }
    if expected.is_some() && !verified {
        return Err(format!("known-count verification failed for N={n}"));
    }
    Ok(())
}

fn duration_percentile(samples: &[Duration], numerator: usize, denominator: usize) -> Duration {
    let position = (samples.len() - 1) * numerator / denominator;
    samples[position]
}

fn phase_total(
    result: &GpuContractionResult,
    select: fn(&nqueens_peps_naive::gpu::GpuLayerMetric) -> Duration,
) -> Duration {
    result.layers.iter().map(select).sum()
}

fn bench(args: &[String]) -> Result<(), String> {
    let max_n = parse_usize(args.first(), "MAX_N")?;
    let mut min_n = 1_usize;
    let mut common_args = Vec::new();
    let mut index = 1;
    while index < args.len() {
        if args[index] == "--min" {
            min_n = parse_usize(args.get(index + 1), "--min value")?;
            index += 2;
        } else {
            common_args.push(args[index].clone());
            index += 1;
        }
    }
    if min_n > max_n {
        return Err("--min must not exceed MAX_N".to_owned());
    }
    let (options, _, warmup, repeats, csv) = parse_common(&common_args, false, true)?;
    let device = probe_device(options.device_id).map_err(|error| error.to_string())?;
    if csv {
        println!(
            "backend,scheme,device_id,device_name,compute_capability,cuda_driver,cuda_runtime,N,count,known_count,verified,median_host_s,min_host_s,p10_host_s,p90_host_s,median_gpu_s,median_count_scan_s,median_expansion_s,median_sort_s,median_run_length_s,median_reduction_s,median_metric_s,peak_host_rss_bytes,peak_device_bytes,peak_support,tensor_entries_examined,tensor_entries_matched,row_operator_candidates,row_operator_matched,warmup,repeats"
        );
    }
    for n in min_n..=max_n {
        for _ in 0..warmup {
            contract_rows_gpu(n, options).map_err(|error| error.to_string())?;
        }
        let mut results = Vec::with_capacity(repeats);
        for _ in 0..repeats {
            results.push(contract_rows_gpu(n, options).map_err(|error| error.to_string())?);
        }
        let result = results.last().expect("repeats is positive");
        let expected = known_count(n);
        let verified = expected == Some(result.count);
        if expected.is_some() && !verified {
            return Err(format!("known-count verification failed for N={n}"));
        }
        let mut host: Vec<_> = results.iter().map(|sample| sample.host_elapsed).collect();
        let mut gpu: Vec<_> = results.iter().map(|sample| sample.gpu_elapsed).collect();
        let mut scan: Vec<_> = results
            .iter()
            .map(|sample| phase_total(sample, |layer| layer.count_scan))
            .collect();
        let mut expansion: Vec<_> = results
            .iter()
            .map(|sample| phase_total(sample, |layer| layer.expansion))
            .collect();
        let mut sort: Vec<_> = results
            .iter()
            .map(|sample| phase_total(sample, |layer| layer.sort))
            .collect();
        let mut rle: Vec<_> = results
            .iter()
            .map(|sample| phase_total(sample, |layer| layer.run_length))
            .collect();
        let mut reduction: Vec<_> = results
            .iter()
            .map(|sample| phase_total(sample, |layer| layer.reduction))
            .collect();
        let mut metric: Vec<_> = results
            .iter()
            .map(|sample| phase_total(sample, |layer| layer.metric))
            .collect();
        for samples in [
            &mut host,
            &mut gpu,
            &mut scan,
            &mut expansion,
            &mut sort,
            &mut rle,
            &mut reduction,
            &mut metric,
        ] {
            samples.sort_unstable();
        }
        let middle = repeats / 2;
        if csv {
            println!(
                "gpu,{},{},{},{}.{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{},{},{},{},{},{},{},{}",
                result.scheme,
                device.device_id,
                csv_text(&device.name),
                device.compute_major,
                device.compute_minor,
                version_text(device.cuda_driver_version),
                version_text(device.cuda_runtime_version),
                n,
                result.count,
                expected.map_or_else(String::new, |count| count.to_string()),
                verified,
                host[middle].as_secs_f64(),
                host[0].as_secs_f64(),
                duration_percentile(&host, 1, 10).as_secs_f64(),
                duration_percentile(&host, 9, 10).as_secs_f64(),
                gpu[middle].as_secs_f64(),
                scan[middle].as_secs_f64(),
                expansion[middle].as_secs_f64(),
                sort[middle].as_secs_f64(),
                rle[middle].as_secs_f64(),
                reduction[middle].as_secs_f64(),
                metric[middle].as_secs_f64(),
                result.peak_host_rss_bytes,
                result.peak_device_bytes,
                result.peak_states,
                result.tensor_entries_examined,
                result.tensor_entries_matched,
                result.row_operator_candidates,
                result.row_operator_matched,
                warmup,
                repeats,
            );
        } else {
            println!(
                "scheme={} N={} Q(N)={} median_host={:.6}s median_gpu={:.6}s peak_device={:.1}MiB peak_support={} verified={}",
                result.scheme,
                n,
                result.count,
                host[middle].as_secs_f64(),
                gpu[middle].as_secs_f64(),
                result.peak_device_bytes as f64 / (1024.0 * 1024.0),
                result.peak_states,
                verified,
            );
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("probe") => probe(&args[1..]),
        Some("self-test") => self_test(&args[1..]),
        Some("solve") => solve(&args[1..]),
        Some("bench") => bench(&args[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}\n{}", usage());
            ExitCode::FAILURE
        }
    }
}
