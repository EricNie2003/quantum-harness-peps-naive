#ifndef NQUEENS_GPU_H
#define NQUEENS_GPU_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    NQ_GPU_SCHEME_COMPACT64 = 1,
    NQ_GPU_SCHEME_WIDE128 = 2,
};

typedef struct NqGpuTransitionDescriptor {
    uint32_t column_in;
    uint32_t column_out;
    uint32_t row_in;
    uint32_t row_out;
    uint32_t diag_dr_in;
    uint32_t diag_dr_out;
    uint32_t diag_dl_in;
    uint32_t diag_dl_out;
    uint64_t value_lo;
    uint64_t value_hi;
    uint64_t tensor_entries_examined;
    uint64_t tensor_entries_matched;
} NqGpuTransitionDescriptor;

typedef struct NqGpuOptions {
    uint32_t device_id;
    uint32_t scheme;
    uint32_t memory_limit_percent;
    uint32_t reserved;
} NqGpuOptions;

typedef struct NqGpuLayerMetric {
    uint64_t row;
    uint64_t input_states;
    uint64_t row_operator_candidates;
    uint64_t row_operator_matched;
    uint64_t completed_row_terms;
    uint64_t output_states;
    uint64_t output_weight_lo;
    uint64_t output_weight_hi;
    uint64_t count_scan_ns;
    uint64_t expansion_ns;
    uint64_t sort_ns;
    uint64_t run_length_ns;
    uint64_t reduction_ns;
    uint64_t metric_ns;
    uint64_t peak_device_bytes;
} NqGpuLayerMetric;

typedef struct NqGpuRunResult {
    uint32_t n;
    uint32_t scheme;
    uint32_t device_id;
    uint32_t compute_major;
    uint32_t compute_minor;
    uint32_t cuda_driver_version;
    uint32_t cuda_runtime_version;
    uint32_t reserved;
    uint64_t count_lo;
    uint64_t count_hi;
    uint64_t host_elapsed_ns;
    uint64_t gpu_elapsed_ns;
    uint64_t peak_device_bytes;
    uint64_t peak_states;
    uint64_t tensor_entries_examined;
    uint64_t tensor_entries_matched;
    uint64_t row_operator_candidates;
    uint64_t row_operator_matched;
    uint64_t layer_count;
    unsigned char device_name[128];
    unsigned char error[512];
} NqGpuRunResult;

typedef struct NqGpuDeviceInfo {
    uint32_t device_id;
    uint32_t compute_major;
    uint32_t compute_minor;
    uint32_t cuda_driver_version;
    uint32_t cuda_runtime_version;
    uint32_t multiprocessor_count;
    uint32_t reserved0;
    uint32_t reserved1;
    uint64_t total_global_memory;
    unsigned char device_name[128];
    unsigned char error[512];
} NqGpuDeviceInfo;

int nq_gpu_probe(uint32_t device_id, NqGpuDeviceInfo* info);

int nq_gpu_self_test(uint32_t device_id, unsigned char* error, size_t error_capacity);

int nq_gpu_contract(
    uint32_t n,
    const NqGpuTransitionDescriptor* descriptor,
    const NqGpuOptions* options,
    NqGpuRunResult* result,
    NqGpuLayerMetric* layers,
    size_t layer_capacity);

#ifdef __cplusplus
}
#endif

#endif
