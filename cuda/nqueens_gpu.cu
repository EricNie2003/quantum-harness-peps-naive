#include "nqueens_gpu.h"

#include <cub/cub.cuh>
#include <cuda_runtime.h>

#include <algorithm>
#include <chrono>
#include <climits>
#include <cstdint>
#include <cstring>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

namespace {

struct U128 {
    uint64_t lo;
    uint64_t hi;

    __host__ __device__ bool operator==(const U128& other) const {
        return lo == other.lo && hi == other.hi;
    }
};

using WideKey = U128;

struct WidePayload {
    uint64_t other_key_word;
    U128 coefficient;
};

static_assert(sizeof(U128) == 16);
static_assert(sizeof(WidePayload) == 24);
static_assert(sizeof(NqGpuTransitionDescriptor) == 64);
static_assert(sizeof(NqGpuOptions) == 16);
static_assert(sizeof(NqGpuLayerMetric) == 120);
static_assert(sizeof(NqGpuRunResult) == 760);
static_assert(sizeof(NqGpuDeviceInfo) == 680);

std::string cuda_message(cudaError_t status, const char* operation) {
    std::ostringstream message;
    message << operation << ": " << cudaGetErrorString(status);
    return message.str();
}

void check_cuda(cudaError_t status, const char* operation) {
    if (status != cudaSuccess) {
        throw std::runtime_error(cuda_message(status, operation));
    }
}

void copy_string(unsigned char* destination, size_t capacity, const std::string& source) {
    if (capacity == 0) {
        return;
    }
    const size_t copied = std::min(capacity - 1, source.size());
    std::memcpy(destination, source.data(), copied);
    destination[copied] = 0;
}

uint64_t checked_add_host(uint64_t left, uint64_t right, const char* label) {
    if (right > std::numeric_limits<uint64_t>::max() - left) {
        throw std::overflow_error(std::string(label) + " overflow");
    }
    return left + right;
}

U128 checked_add_host(U128 left, U128 right, const char* label) {
    U128 result{};
    result.lo = left.lo + right.lo;
    const uint64_t carry = result.lo < left.lo ? 1 : 0;
    const uint64_t high_without_carry = left.hi + right.hi;
    if (high_without_carry < left.hi) {
        throw std::overflow_error(std::string(label) + " overflow");
    }
    result.hi = high_without_carry + carry;
    if (result.hi < high_without_carry) {
        throw std::overflow_error(std::string(label) + " overflow");
    }
    return result;
}

class MemoryTracker {
  public:
    MemoryTracker(uint64_t total_memory, uint32_t limit_percent)
        : limit_bytes_(total_memory / 100 * limit_percent) {}

    void before_allocate(uint64_t bytes, const char* label) {
        if (bytes > limit_bytes_ || current_bytes_ > limit_bytes_ - bytes) {
            std::ostringstream message;
            message << "GPU memory guard rejected " << label << ": requested=" << bytes
                    << " current=" << current_bytes_ << " limit=" << limit_bytes_;
            throw std::runtime_error(message.str());
        }
    }

    void allocated(uint64_t bytes) {
        current_bytes_ += bytes;
        peak_bytes_ = std::max(peak_bytes_, current_bytes_);
    }

    void freed(uint64_t bytes) {
        current_bytes_ -= bytes;
    }

    uint64_t peak_bytes() const { return peak_bytes_; }

  private:
    uint64_t limit_bytes_ = 0;
    uint64_t current_bytes_ = 0;
    uint64_t peak_bytes_ = 0;
};

template <typename T> class DeviceBuffer {
  public:
    DeviceBuffer() = default;

    DeviceBuffer(MemoryTracker& tracker, size_t count, const char* label) {
        allocate(tracker, count, label);
    }

    DeviceBuffer(const DeviceBuffer&) = delete;
    DeviceBuffer& operator=(const DeviceBuffer&) = delete;

    DeviceBuffer(DeviceBuffer&& other) noexcept { move_from(other); }

    DeviceBuffer& operator=(DeviceBuffer&& other) noexcept {
        if (this != &other) {
            reset();
            move_from(other);
        }
        return *this;
    }

    ~DeviceBuffer() { reset(); }

    void allocate(MemoryTracker& tracker, size_t count, const char* label) {
        reset();
        if (count == 0) {
            return;
        }
        if (count > std::numeric_limits<size_t>::max() / sizeof(T)) {
            throw std::overflow_error(std::string(label) + " byte size overflow");
        }
        const uint64_t bytes = static_cast<uint64_t>(count * sizeof(T));
        tracker.before_allocate(bytes, label);
        check_cuda(cudaMalloc(reinterpret_cast<void**>(&pointer_), bytes), label);
        tracker_ = &tracker;
        count_ = count;
        bytes_ = bytes;
        tracker.allocated(bytes);
    }

    void reset() noexcept {
        if (pointer_ != nullptr) {
            cudaFree(pointer_);
            tracker_->freed(bytes_);
        }
        pointer_ = nullptr;
        tracker_ = nullptr;
        count_ = 0;
        bytes_ = 0;
    }

    T* get() { return pointer_; }
    const T* get() const { return pointer_; }
    size_t size() const { return count_; }

  private:
    void move_from(DeviceBuffer& other) noexcept {
        pointer_ = std::exchange(other.pointer_, nullptr);
        tracker_ = std::exchange(other.tracker_, nullptr);
        count_ = std::exchange(other.count_, 0);
        bytes_ = std::exchange(other.bytes_, 0);
    }

    T* pointer_ = nullptr;
    MemoryTracker* tracker_ = nullptr;
    size_t count_ = 0;
    uint64_t bytes_ = 0;
};

template <typename Function> uint64_t time_cuda_phase(Function&& function) {
    cudaEvent_t start = nullptr;
    cudaEvent_t stop = nullptr;
    check_cuda(cudaEventCreate(&start), "cudaEventCreate(start)");
    try {
        check_cuda(cudaEventCreate(&stop), "cudaEventCreate(stop)");
        check_cuda(cudaEventRecord(start), "cudaEventRecord(start)");
        function();
        check_cuda(cudaEventRecord(stop), "cudaEventRecord(stop)");
        check_cuda(cudaEventSynchronize(stop), "cudaEventSynchronize(stop)");
        float milliseconds = 0.0F;
        check_cuda(cudaEventElapsedTime(&milliseconds, start, stop), "cudaEventElapsedTime");
        cudaEventDestroy(stop);
        cudaEventDestroy(start);
        return static_cast<uint64_t>(milliseconds * 1'000'000.0F);
    } catch (...) {
        if (stop != nullptr) {
            cudaEventDestroy(stop);
        }
        cudaEventDestroy(start);
        throw;
    }
}

__host__ __device__ uint64_t mask_for_n(uint32_t n) {
    return n == 64 ? std::numeric_limits<uint64_t>::max() : ((uint64_t{1} << n) - 1);
}

__host__ __device__ uint32_t bit_at(uint64_t value, uint32_t index) {
    return static_cast<uint32_t>((value >> index) & 1U);
}

__host__ __device__ uint64_t replace_bit(uint64_t value, uint32_t index, uint32_t bit) {
    const uint64_t selected = uint64_t{1} << index;
    return (value & ~selected) | (static_cast<uint64_t>(bit) << index);
}

__host__ __device__ void insert_field(WideKey& key, uint64_t value, uint32_t shift, uint32_t width) {
    if (shift < 64) {
        key.lo |= value << shift;
        if (shift != 0 && shift + width > 64) {
            key.hi |= value >> (64 - shift);
        }
    } else {
        key.hi |= value << (shift - 64);
    }
}

__host__ __device__ uint64_t extract_field(WideKey key, uint32_t shift, uint32_t width) {
    uint64_t value = 0;
    if (shift < 64) {
        value = key.lo >> shift;
        if (shift != 0 && shift + width > 64) {
            value |= key.hi << (64 - shift);
        }
    } else {
        value = key.hi >> (shift - 64);
    }
    return value & mask_for_n(width);
}

template <typename Key> struct KeyOperations;

template <> struct KeyOperations<uint64_t> {
    __host__ __device__ static void unpack(
        uint64_t key, uint32_t n, uint64_t& columns, uint64_t& diag_dr, uint64_t& diag_dl) {
        const uint64_t mask = mask_for_n(n);
        columns = key & mask;
        diag_dr = (key >> n) & mask;
        diag_dl = (key >> (2 * n)) & mask;
    }

    __host__ __device__ static uint64_t pack(
        uint64_t columns, uint64_t diag_dr, uint64_t diag_dl, uint32_t n) {
        return columns | (diag_dr << n) | (diag_dl << (2 * n));
    }

    __host__ __device__ static uint64_t columns(uint64_t key, uint32_t n) {
        return key & mask_for_n(n);
    }
};

template <> struct KeyOperations<WideKey> {
    __host__ __device__ static void unpack(
        WideKey key, uint32_t n, uint64_t& columns, uint64_t& diag_dr, uint64_t& diag_dl) {
        columns = extract_field(key, 0, n);
        diag_dr = extract_field(key, n, n);
        diag_dl = extract_field(key, 2 * n, n);
    }

    __host__ __device__ static WideKey pack(
        uint64_t columns, uint64_t diag_dr, uint64_t diag_dl, uint32_t n) {
        WideKey key{};
        insert_field(key, columns, 0, n);
        insert_field(key, diag_dr, n, n);
        insert_field(key, diag_dl, 2 * n, n);
        return key;
    }

    __host__ __device__ static uint64_t columns(WideKey key, uint32_t n) {
        return extract_field(key, 0, n);
    }
};

template <typename Coefficient> struct CoefficientOperations;

template <> struct CoefficientOperations<uint64_t> {
    __device__ static uint64_t add(uint64_t left, uint64_t right, int* overflow) {
        if (right > std::numeric_limits<uint64_t>::max() - left) {
            atomicExch(overflow, 1);
        }
        return left + right;
    }

    __host__ __device__ static U128 widen(uint64_t value) { return U128{value, 0}; }
};

template <> struct CoefficientOperations<U128> {
    __device__ static U128 add(U128 left, U128 right, int* overflow) {
        U128 result{};
        result.lo = left.lo + right.lo;
        const uint64_t carry = result.lo < left.lo ? 1 : 0;
        const uint64_t high_without_carry = left.hi + right.hi;
        if (high_without_carry < left.hi) {
            atomicExch(overflow, 1);
        }
        result.hi = high_without_carry + carry;
        if (result.hi < high_without_carry) {
            atomicExch(overflow, 1);
        }
        return result;
    }

    __host__ __device__ static U128 widen(U128 value) { return value; }
};

__device__ U128 add_wide_metric(U128 left, U128 right, int* overflow) {
    return CoefficientOperations<U128>::add(left, right, overflow);
}

__device__ bool transition_matches(
    uint64_t columns,
    uint64_t diag_dr,
    uint64_t diag_dl,
    uint32_t column,
    const NqGpuTransitionDescriptor& descriptor) {
    return bit_at(columns, column) == descriptor.column_in
        && bit_at(diag_dr, column) == descriptor.diag_dr_in
        && bit_at(diag_dl, column) == descriptor.diag_dl_in
        && descriptor.row_in == 0
        && descriptor.row_out == 1;
}

template <typename Key>
__global__ void count_candidates_kernel(
    const Key* parents,
    uint64_t parent_count,
    uint32_t n,
    NqGpuTransitionDescriptor descriptor,
    uint64_t* counts) {
    const uint64_t index = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= parent_count) {
        return;
    }
    uint64_t columns = 0;
    uint64_t diag_dr = 0;
    uint64_t diag_dl = 0;
    KeyOperations<Key>::unpack(parents[index], n, columns, diag_dr, diag_dl);
    uint64_t count = 0;
    for (uint32_t column = 0; column < n; ++column) {
        count += transition_matches(columns, diag_dr, diag_dl, column, descriptor) ? 1 : 0;
    }
    counts[index] = count;
}

__global__ void finish_scan_kernel(
    const uint64_t* offsets, const uint64_t* counts, uint64_t count, uint64_t* total) {
    if (blockIdx.x == 0 && threadIdx.x == 0) {
        *total = count == 0 ? 0 : offsets[count - 1] + counts[count - 1];
    }
}

template <typename Key, typename Coefficient>
__global__ void fill_candidates_kernel(
    const Key* parents,
    const Coefficient* parent_coefficients,
    uint64_t parent_count,
    uint32_t n,
    NqGpuTransitionDescriptor descriptor,
    const uint64_t* offsets,
    Key* candidate_keys,
    Coefficient* candidate_coefficients) {
    const uint64_t index = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= parent_count) {
        return;
    }
    uint64_t columns = 0;
    uint64_t diag_dr = 0;
    uint64_t diag_dl = 0;
    KeyOperations<Key>::unpack(parents[index], n, columns, diag_dr, diag_dl);
    const uint64_t board_mask = mask_for_n(n);
    uint64_t output = offsets[index];
    for (uint32_t column = 0; column < n; ++column) {
        if (!transition_matches(columns, diag_dr, diag_dl, column, descriptor)) {
            continue;
        }
        const uint64_t columns_out = replace_bit(columns, column, descriptor.column_out);
        const uint64_t diag_dr_sites = replace_bit(diag_dr, column, descriptor.diag_dr_out);
        const uint64_t diag_dl_sites = replace_bit(diag_dl, column, descriptor.diag_dl_out);
        candidate_keys[output] = KeyOperations<Key>::pack(
            columns_out, (diag_dr_sites << 1) & board_mask, diag_dl_sites >> 1, n);
        candidate_coefficients[output] = parent_coefficients[index];
        ++output;
    }
}

__global__ void split_wide_records_kernel(
    const WideKey* keys,
    const U128* coefficients,
    uint64_t count,
    uint64_t* low_words,
    WidePayload* payloads) {
    const uint64_t index = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < count) {
        low_words[index] = keys[index].lo;
        payloads[index] = WidePayload{keys[index].hi, coefficients[index]};
    }
}

__global__ void prepare_high_sort_kernel(
    const uint64_t* sorted_low,
    const WidePayload* low_payload,
    uint64_t count,
    uint64_t* high_words,
    WidePayload* high_payload) {
    const uint64_t index = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < count) {
        high_words[index] = low_payload[index].other_key_word;
        high_payload[index] = WidePayload{sorted_low[index], low_payload[index].coefficient};
    }
}

__global__ void join_wide_records_kernel(
    const uint64_t* primary_words,
    const WidePayload* payloads,
    uint64_t count,
    bool primary_is_high,
    WideKey* keys,
    U128* coefficients) {
    const uint64_t index = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < count) {
        if (primary_is_high) {
            keys[index] = WideKey{payloads[index].other_key_word, primary_words[index]};
        } else {
            keys[index] = WideKey{primary_words[index], payloads[index].other_key_word};
        }
        coefficients[index] = payloads[index].coefficient;
    }
}

__global__ void arithmetic_self_test_kernel(int* flags, U128* carry_result) {
    if (blockIdx.x != 0 || threadIdx.x != 0) {
        return;
    }
    flags[0] = 0;
    flags[1] = 0;
    flags[2] = 0;
    (void)CoefficientOperations<uint64_t>::add(
        std::numeric_limits<uint64_t>::max(), 1, &flags[0]);
    *carry_result = CoefficientOperations<U128>::add(
        U128{std::numeric_limits<uint64_t>::max(), 0}, U128{1, 0}, &flags[1]);
    (void)CoefficientOperations<U128>::add(
        U128{std::numeric_limits<uint64_t>::max(), std::numeric_limits<uint64_t>::max()},
        U128{1, 0},
        &flags[2]);
}

template <typename Key, typename Coefficient>
__global__ void reduce_runs_kernel(
    const Key* unique_keys,
    const uint64_t* run_lengths,
    const uint64_t* run_offsets,
    uint64_t run_count,
    const Coefficient* sorted_coefficients,
    Key* output_keys,
    Coefficient* output_coefficients,
    int* overflow) {
    const uint64_t run = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (run >= run_count) {
        return;
    }
    const uint64_t offset = run_offsets[run];
    const uint64_t length = run_lengths[run];
    Coefficient sum{};
    for (uint64_t index = 0; index < length; ++index) {
        sum = CoefficientOperations<Coefficient>::add(sum, sorted_coefficients[offset + index], overflow);
    }
    output_keys[run] = unique_keys[run];
    output_coefficients[run] = sum;
}

template <typename Key, typename Coefficient>
__global__ void sum_blocks_kernel(
    const Key* keys,
    const Coefficient* coefficients,
    uint64_t count,
    uint32_t n,
    bool filter_columns,
    U128* block_sums,
    int* overflow) {
    extern __shared__ U128 shared[];
    const uint32_t lane = threadIdx.x;
    const uint64_t start = static_cast<uint64_t>(blockIdx.x) * blockDim.x + lane;
    const uint64_t stride = static_cast<uint64_t>(gridDim.x) * blockDim.x;
    U128 local{};
    const uint64_t board_mask = mask_for_n(n);
    for (uint64_t index = start; index < count; index += stride) {
        if (!filter_columns || KeyOperations<Key>::columns(keys[index], n) == board_mask) {
            local = add_wide_metric(
                local, CoefficientOperations<Coefficient>::widen(coefficients[index]), overflow);
        }
    }
    shared[lane] = local;
    __syncthreads();
    for (uint32_t offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (lane < offset) {
            shared[lane] = add_wide_metric(shared[lane], shared[lane + offset], overflow);
        }
        __syncthreads();
    }
    if (lane == 0) {
        block_sums[blockIdx.x] = shared[0];
    }
}

uint32_t grid_size(uint64_t items, uint32_t block_size = 256) {
    const uint64_t blocks = (items + block_size - 1) / block_size;
    if (blocks > std::numeric_limits<uint32_t>::max()) {
        throw std::runtime_error("CUDA grid dimension exceeds uint32");
    }
    return static_cast<uint32_t>(blocks);
}

void verify_kernel(const char* label) {
    check_cuda(cudaGetLastError(), label);
}

template <typename Key, typename Coefficient>
U128 compute_sum(
    MemoryTracker& tracker,
    const Key* keys,
    const Coefficient* coefficients,
    uint64_t count,
    uint32_t n,
    bool filter_columns,
    DeviceBuffer<int>& overflow) {
    if (count == 0) {
        return U128{};
    }
    constexpr uint32_t block_size = 256;
    const uint32_t blocks = std::min<uint32_t>(grid_size(count, block_size), 65'535);
    DeviceBuffer<U128> partials(tracker, blocks, "allocate metric partial sums");
    check_cuda(cudaMemset(overflow.get(), 0, sizeof(int)), "clear metric overflow flag");
    sum_blocks_kernel<<<blocks, block_size, block_size * sizeof(U128)>>>(
        keys, coefficients, count, n, filter_columns, partials.get(), overflow.get());
    verify_kernel("launch metric sum kernel");
    std::vector<U128> host_partials(blocks);
    check_cuda(
        cudaMemcpy(
            host_partials.data(), partials.get(), blocks * sizeof(U128), cudaMemcpyDeviceToHost),
        "copy metric partial sums");
    int overflow_value = 0;
    check_cuda(
        cudaMemcpy(&overflow_value, overflow.get(), sizeof(int), cudaMemcpyDeviceToHost),
        "copy metric overflow flag");
    if (overflow_value != 0) {
        throw std::overflow_error("128-bit metric accumulation overflow");
    }
    U128 total{};
    for (const U128 value : host_partials) {
        total = checked_add_host(total, value, "host metric accumulation");
    }
    return total;
}

void sort_compact(
    MemoryTracker& tracker,
    uint64_t* input_keys,
    uint64_t* input_coefficients,
    uint64_t count,
    uint32_t significant_bits,
    uint64_t* output_keys,
    uint64_t* output_coefficients) {
    size_t temp_bytes = 0;
    check_cuda(
        cub::DeviceRadixSort::SortPairs(
            nullptr,
            temp_bytes,
            input_keys,
            output_keys,
            input_coefficients,
            output_coefficients,
            static_cast<int>(count),
            0,
            static_cast<int>(significant_bits)),
        "query compact radix-sort storage");
    DeviceBuffer<unsigned char> temp(tracker, temp_bytes, "allocate compact radix-sort storage");
    check_cuda(
        cub::DeviceRadixSort::SortPairs(
            temp.get(),
            temp_bytes,
            input_keys,
            output_keys,
            input_coefficients,
            output_coefficients,
            static_cast<int>(count),
            0,
            static_cast<int>(significant_bits)),
        "compact radix sort");
}

void sort_wide(
    MemoryTracker& tracker,
    WideKey* keys,
    U128* coefficients,
    uint64_t count,
    uint32_t significant_bits) {
    DeviceBuffer<uint64_t> words_a(tracker, count, "allocate wide radix words A");
    DeviceBuffer<uint64_t> words_b(tracker, count, "allocate wide radix words B");
    DeviceBuffer<WidePayload> payload_a(tracker, count, "allocate wide radix payload A");
    DeviceBuffer<WidePayload> payload_b(tracker, count, "allocate wide radix payload B");
    const uint32_t blocks = grid_size(count);
    split_wide_records_kernel<<<blocks, 256>>>(
        keys, coefficients, count, words_a.get(), payload_a.get());
    verify_kernel("launch wide-key split kernel");

    size_t low_temp_bytes = 0;
    check_cuda(
        cub::DeviceRadixSort::SortPairs(
            nullptr,
            low_temp_bytes,
            words_a.get(),
            words_b.get(),
            payload_a.get(),
            payload_b.get(),
            static_cast<int>(count),
            0,
            static_cast<int>(std::min<uint32_t>(64, significant_bits))),
        "query wide low-word radix-sort storage");
    size_t high_temp_bytes = 0;
    const uint32_t high_bits = significant_bits > 64 ? significant_bits - 64 : 0;
    if (high_bits != 0) {
        check_cuda(
            cub::DeviceRadixSort::SortPairs(
                nullptr,
                high_temp_bytes,
                words_a.get(),
                words_b.get(),
                payload_a.get(),
                payload_b.get(),
                static_cast<int>(count),
                0,
                static_cast<int>(high_bits)),
            "query wide high-word radix-sort storage");
    }
    const size_t temp_bytes = std::max(low_temp_bytes, high_temp_bytes);
    DeviceBuffer<unsigned char> temp(
        tracker, temp_bytes, "allocate reusable wide radix-sort storage");
    check_cuda(
        cub::DeviceRadixSort::SortPairs(
            temp.get(),
            low_temp_bytes,
            words_a.get(),
            words_b.get(),
            payload_a.get(),
            payload_b.get(),
            static_cast<int>(count),
            0,
            static_cast<int>(std::min<uint32_t>(64, significant_bits))),
        "wide low-word radix sort");

    if (significant_bits <= 64) {
        join_wide_records_kernel<<<blocks, 256>>>(
            words_b.get(), payload_b.get(), count, false, keys, coefficients);
        verify_kernel("launch compact-range wide-key join kernel");
        return;
    }

    prepare_high_sort_kernel<<<blocks, 256>>>(
        words_b.get(), payload_b.get(), count, words_a.get(), payload_a.get());
    verify_kernel("launch wide high-word preparation kernel");
    check_cuda(
        cub::DeviceRadixSort::SortPairs(
            temp.get(),
            high_temp_bytes,
            words_a.get(),
            words_b.get(),
            payload_a.get(),
            payload_b.get(),
            static_cast<int>(count),
            0,
            static_cast<int>(high_bits)),
        "wide high-word radix sort");
    join_wide_records_kernel<<<blocks, 256>>>(
        words_b.get(), payload_b.get(), count, true, keys, coefficients);
    verify_kernel("launch wide-key join kernel");
}

template <typename Key, typename Coefficient>
U128 run_contraction(
    uint32_t n,
    const NqGpuTransitionDescriptor& descriptor,
    MemoryTracker& tracker,
    NqGpuRunResult& result,
    NqGpuLayerMetric* layer_output) {
    DeviceBuffer<Key> boundary_keys(tracker, 1, "allocate initial boundary key");
    DeviceBuffer<Coefficient> boundary_coefficients(
        tracker, 1, "allocate initial boundary coefficient");
    const Key zero_key{};
    const Coefficient one_coefficient = [] {
        if constexpr (std::is_same_v<Coefficient, uint64_t>) {
            return uint64_t{1};
        } else {
            return U128{1, 0};
        }
    }();
    check_cuda(
        cudaMemcpy(boundary_keys.get(), &zero_key, sizeof(Key), cudaMemcpyHostToDevice),
        "copy initial boundary key");
    check_cuda(
        cudaMemcpy(
            boundary_coefficients.get(),
            &one_coefficient,
            sizeof(Coefficient),
            cudaMemcpyHostToDevice),
        "copy initial boundary coefficient");
    DeviceBuffer<int> overflow(tracker, 1, "allocate overflow flag");

    uint64_t boundary_count = 1;
    uint64_t total_candidates = 0;
    uint64_t total_matched = 0;
    uint64_t gpu_elapsed_ns = 0;
    result.peak_states = 1;

    for (uint32_t row = 0; row < n; ++row) {
        NqGpuLayerMetric& metric = layer_output[row];
        std::memset(&metric, 0, sizeof(metric));
        metric.row = row;
        metric.input_states = boundary_count;
        if (boundary_count == 0) {
            metric.peak_device_bytes = tracker.peak_bytes();
            continue;
        }
        if (boundary_count > static_cast<uint64_t>(INT_MAX)) {
            throw std::runtime_error("boundary support exceeds the CUB INT_MAX item limit");
        }

        DeviceBuffer<uint64_t> candidate_counts(
            tracker, boundary_count, "allocate candidate counts");
        DeviceBuffer<uint64_t> candidate_offsets(
            tracker, boundary_count, "allocate candidate offsets");
        DeviceBuffer<uint64_t> candidate_total(tracker, 1, "allocate candidate total");
        metric.count_scan_ns = time_cuda_phase([&] {
            count_candidates_kernel<<<grid_size(boundary_count), 256>>>(
                boundary_keys.get(), boundary_count, n, descriptor, candidate_counts.get());
            verify_kernel("launch candidate-count kernel");
            size_t scan_temp_bytes = 0;
            check_cuda(
                cub::DeviceScan::ExclusiveSum(
                    nullptr,
                    scan_temp_bytes,
                    candidate_counts.get(),
                    candidate_offsets.get(),
                    static_cast<int>(boundary_count)),
                "query candidate-scan storage");
            DeviceBuffer<unsigned char> scan_temp(
                tracker, scan_temp_bytes, "allocate candidate-scan storage");
            check_cuda(
                cub::DeviceScan::ExclusiveSum(
                    scan_temp.get(),
                    scan_temp_bytes,
                    candidate_counts.get(),
                    candidate_offsets.get(),
                    static_cast<int>(boundary_count)),
                "scan candidate counts");
            finish_scan_kernel<<<1, 1>>>(
                candidate_offsets.get(),
                candidate_counts.get(),
                boundary_count,
                candidate_total.get());
            verify_kernel("launch candidate-total kernel");
        });
        uint64_t candidate_count = 0;
        check_cuda(
            cudaMemcpy(
                &candidate_count,
                candidate_total.get(),
                sizeof(candidate_count),
                cudaMemcpyDeviceToHost),
            "copy candidate total");
        if (candidate_count > static_cast<uint64_t>(INT_MAX)) {
            throw std::runtime_error("candidate count exceeds the CUB INT_MAX item limit");
        }
        metric.row_operator_candidates = boundary_count * static_cast<uint64_t>(n);
        metric.row_operator_matched = candidate_count;
        metric.completed_row_terms = candidate_count;
        total_candidates = checked_add_host(
            total_candidates, metric.row_operator_candidates, "row-operator candidate metric");
        total_matched = checked_add_host(
            total_matched, metric.row_operator_matched, "row-operator matched metric");

        if (candidate_count == 0) {
            boundary_keys.reset();
            boundary_coefficients.reset();
            boundary_count = 0;
            metric.output_states = 0;
            metric.peak_device_bytes = tracker.peak_bytes();
            gpu_elapsed_ns = checked_add_host(
                gpu_elapsed_ns, metric.count_scan_ns, "GPU elapsed metric");
            continue;
        }

        DeviceBuffer<Key> candidate_keys(tracker, candidate_count, "allocate candidate keys");
        DeviceBuffer<Coefficient> candidate_coefficients(
            tracker, candidate_count, "allocate candidate coefficients");
        metric.expansion_ns = time_cuda_phase([&] {
            fill_candidates_kernel<<<grid_size(boundary_count), 256>>>(
                boundary_keys.get(),
                boundary_coefficients.get(),
                boundary_count,
                n,
                descriptor,
                candidate_offsets.get(),
                candidate_keys.get(),
                candidate_coefficients.get());
            verify_kernel("launch candidate-fill kernel");
        });
        boundary_keys.reset();
        boundary_coefficients.reset();
        candidate_counts.reset();
        candidate_offsets.reset();
        candidate_total.reset();

        if constexpr (std::is_same_v<Key, uint64_t>) {
            DeviceBuffer<uint64_t> sorted_keys(
                tracker, candidate_count, "allocate compact sorted keys");
            DeviceBuffer<uint64_t> sorted_coefficients(
                tracker, candidate_count, "allocate compact sorted coefficients");
            metric.sort_ns = time_cuda_phase([&] {
                sort_compact(
                    tracker,
                    candidate_keys.get(),
                    candidate_coefficients.get(),
                    candidate_count,
                    3 * n,
                    sorted_keys.get(),
                    sorted_coefficients.get());
            });
            candidate_keys = std::move(sorted_keys);
            candidate_coefficients = std::move(sorted_coefficients);
        } else {
            metric.sort_ns = time_cuda_phase(
                [&] { sort_wide(tracker, candidate_keys.get(), candidate_coefficients.get(), candidate_count, 3 * n); });
        }

        DeviceBuffer<Key> unique_keys(tracker, candidate_count, "allocate unique keys");
        DeviceBuffer<uint64_t> run_lengths(tracker, candidate_count, "allocate run lengths");
        DeviceBuffer<int> run_count_device(tracker, 1, "allocate run count");
        metric.run_length_ns = time_cuda_phase([&] {
            size_t rle_temp_bytes = 0;
            check_cuda(
                cub::DeviceRunLengthEncode::Encode(
                    nullptr,
                    rle_temp_bytes,
                    candidate_keys.get(),
                    unique_keys.get(),
                    run_lengths.get(),
                    run_count_device.get(),
                    static_cast<int>(candidate_count)),
                "query run-length storage");
            DeviceBuffer<unsigned char> rle_temp(
                tracker, rle_temp_bytes, "allocate run-length storage");
            check_cuda(
                cub::DeviceRunLengthEncode::Encode(
                    rle_temp.get(),
                    rle_temp_bytes,
                    candidate_keys.get(),
                    unique_keys.get(),
                    run_lengths.get(),
                    run_count_device.get(),
                    static_cast<int>(candidate_count)),
                "run-length encode sorted keys");
        });
        int run_count_int = 0;
        check_cuda(
            cudaMemcpy(
                &run_count_int,
                run_count_device.get(),
                sizeof(run_count_int),
                cudaMemcpyDeviceToHost),
            "copy run count");
        candidate_keys.reset();
        run_count_device.reset();
        if (run_count_int < 0) {
            throw std::runtime_error("CUB returned a negative run count");
        }
        const uint64_t run_count = static_cast<uint64_t>(run_count_int);
        DeviceBuffer<uint64_t> run_offsets(tracker, run_count, "allocate run offsets");
        DeviceBuffer<Coefficient> reduced_coefficients(
            tracker, run_count, "allocate reduced coefficients");
        DeviceBuffer<Key> reduced_keys(tracker, run_count, "allocate reduced keys");
        check_cuda(cudaMemset(overflow.get(), 0, sizeof(int)), "clear reduction overflow flag");
        metric.reduction_ns = time_cuda_phase([&] {
            size_t run_scan_temp_bytes = 0;
            check_cuda(
                cub::DeviceScan::ExclusiveSum(
                    nullptr,
                    run_scan_temp_bytes,
                    run_lengths.get(),
                    run_offsets.get(),
                    run_count_int),
                "query run-offset storage");
            DeviceBuffer<unsigned char> run_scan_temp(
                tracker, run_scan_temp_bytes, "allocate run-offset storage");
            check_cuda(
                cub::DeviceScan::ExclusiveSum(
                    run_scan_temp.get(),
                    run_scan_temp_bytes,
                    run_lengths.get(),
                    run_offsets.get(),
                    run_count_int),
                "scan run lengths");
            reduce_runs_kernel<<<grid_size(run_count), 256>>>(
                unique_keys.get(),
                run_lengths.get(),
                run_offsets.get(),
                run_count,
                candidate_coefficients.get(),
                reduced_keys.get(),
                reduced_coefficients.get(),
                overflow.get());
            verify_kernel("launch exact run-reduction kernel");
        });
        int overflow_value = 0;
        check_cuda(
            cudaMemcpy(
                &overflow_value, overflow.get(), sizeof(int), cudaMemcpyDeviceToHost),
            "copy reduction overflow flag");
        if (overflow_value != 0) {
            throw std::overflow_error("coefficient overflow during GPU run reduction");
        }
        candidate_coefficients.reset();
        unique_keys.reset();
        run_lengths.reset();
        run_offsets.reset();

        boundary_keys = std::move(reduced_keys);
        boundary_coefficients = std::move(reduced_coefficients);
        boundary_count = run_count;
        metric.output_states = boundary_count;
        result.peak_states = std::max(result.peak_states, boundary_count);
        U128 output_weight{};
        metric.metric_ns = time_cuda_phase([&] {
            output_weight = compute_sum(
                tracker,
                boundary_keys.get(),
                boundary_coefficients.get(),
                boundary_count,
                n,
                false,
                overflow);
        });
        if constexpr (std::is_same_v<Coefficient, uint64_t>) {
            if (output_weight.hi != 0) {
                throw std::overflow_error("compact64 layer output weight exceeds u64");
            }
        }
        metric.output_weight_lo = output_weight.lo;
        metric.output_weight_hi = output_weight.hi;
        metric.peak_device_bytes = tracker.peak_bytes();
        gpu_elapsed_ns = checked_add_host(gpu_elapsed_ns, metric.count_scan_ns, "GPU elapsed metric");
        gpu_elapsed_ns = checked_add_host(gpu_elapsed_ns, metric.expansion_ns, "GPU elapsed metric");
        gpu_elapsed_ns = checked_add_host(gpu_elapsed_ns, metric.sort_ns, "GPU elapsed metric");
        gpu_elapsed_ns = checked_add_host(gpu_elapsed_ns, metric.run_length_ns, "GPU elapsed metric");
        gpu_elapsed_ns = checked_add_host(gpu_elapsed_ns, metric.reduction_ns, "GPU elapsed metric");
        gpu_elapsed_ns = checked_add_host(gpu_elapsed_ns, metric.metric_ns, "GPU elapsed metric");
    }

    result.row_operator_candidates = total_candidates;
    result.row_operator_matched = total_matched;
    result.gpu_elapsed_ns = gpu_elapsed_ns;
    if (boundary_count == 0) {
        return U128{};
    }
    U128 count = compute_sum(
        tracker,
        boundary_keys.get(),
        boundary_coefficients.get(),
        boundary_count,
        n,
        true,
        overflow);
    if constexpr (std::is_same_v<Coefficient, uint64_t>) {
        if (count.hi != 0) {
            throw std::overflow_error("compact64 final count exceeds u64");
        }
    }
    return count;
}

void populate_device_fields(uint32_t device_id, NqGpuRunResult& result, uint64_t& total_memory) {
    check_cuda(cudaSetDevice(static_cast<int>(device_id)), "cudaSetDevice");
    cudaDeviceProp properties{};
    check_cuda(
        cudaGetDeviceProperties(&properties, static_cast<int>(device_id)),
        "cudaGetDeviceProperties");
    int driver_version = 0;
    int runtime_version = 0;
    check_cuda(cudaDriverGetVersion(&driver_version), "cudaDriverGetVersion");
    check_cuda(cudaRuntimeGetVersion(&runtime_version), "cudaRuntimeGetVersion");
    result.device_id = device_id;
    result.compute_major = static_cast<uint32_t>(properties.major);
    result.compute_minor = static_cast<uint32_t>(properties.minor);
    result.cuda_driver_version = static_cast<uint32_t>(driver_version);
    result.cuda_runtime_version = static_cast<uint32_t>(runtime_version);
    copy_string(result.device_name, sizeof(result.device_name), properties.name);
    total_memory = static_cast<uint64_t>(properties.totalGlobalMem);
}

void validate_descriptor(const NqGpuTransitionDescriptor& descriptor) {
    const uint32_t bits[] = {
        descriptor.column_in,
        descriptor.column_out,
        descriptor.row_in,
        descriptor.row_out,
        descriptor.diag_dr_in,
        descriptor.diag_dr_out,
        descriptor.diag_dl_in,
        descriptor.diag_dl_out,
    };
    for (const uint32_t bit : bits) {
        if (bit > 1) {
            throw std::runtime_error("GPU transition descriptor contains a non-binary leg");
        }
    }
    if (descriptor.column_in != 0 || descriptor.column_out != 1
        || descriptor.row_in != 0 || descriptor.row_out != 1
        || descriptor.diag_dr_in != 0 || descriptor.diag_dr_out != 1
        || descriptor.diag_dl_in != 0 || descriptor.diag_dl_out != 1) {
        throw std::runtime_error("GPU descriptor is not the Sec. VI occupied transition");
    }
    if (descriptor.value_lo != 1 || descriptor.value_hi != 0) {
        throw std::runtime_error("GPU descriptor requires a unit occupied coefficient");
    }
    if (descriptor.tensor_entries_examined != 17 || descriptor.tensor_entries_matched != 17) {
        throw std::runtime_error("GPU descriptor was not compiled from all 17 entries of C");
    }
}

void run_device_self_test(uint32_t device_id) {
    check_cuda(cudaSetDevice(static_cast<int>(device_id)), "cudaSetDevice(self-test)");
    cudaDeviceProp properties{};
    check_cuda(
        cudaGetDeviceProperties(&properties, static_cast<int>(device_id)),
        "cudaGetDeviceProperties(self-test)");
    MemoryTracker tracker(static_cast<uint64_t>(properties.totalGlobalMem), 95);

    const WideKey host_keys[] = {
        WideKey{5, 1},
        WideKey{3, 1},
        WideKey{7, 0},
        WideKey{3, 1},
    };
    const U128 host_values[] = {
        U128{11, 0},
        U128{12, 0},
        U128{13, 0},
        U128{14, 0},
    };
    constexpr uint64_t item_count = 4;
    DeviceBuffer<WideKey> keys(tracker, item_count, "allocate self-test keys");
    DeviceBuffer<U128> values(tracker, item_count, "allocate self-test values");
    check_cuda(
        cudaMemcpy(keys.get(), host_keys, sizeof(host_keys), cudaMemcpyHostToDevice),
        "copy self-test keys");
    check_cuda(
        cudaMemcpy(values.get(), host_values, sizeof(host_values), cudaMemcpyHostToDevice),
        "copy self-test values");
    sort_wide(tracker, keys.get(), values.get(), item_count, 128);
    check_cuda(cudaDeviceSynchronize(), "synchronize wide sort self-test");
    WideKey sorted_keys[item_count]{};
    U128 sorted_values[item_count]{};
    check_cuda(
        cudaMemcpy(sorted_keys, keys.get(), sizeof(sorted_keys), cudaMemcpyDeviceToHost),
        "copy sorted self-test keys");
    check_cuda(
        cudaMemcpy(sorted_values, values.get(), sizeof(sorted_values), cudaMemcpyDeviceToHost),
        "copy sorted self-test values");
    const WideKey expected_keys[] = {
        WideKey{7, 0},
        WideKey{3, 1},
        WideKey{3, 1},
        WideKey{5, 1},
    };
    const uint64_t expected_values[] = {13, 12, 14, 11};
    for (size_t index = 0; index < item_count; ++index) {
        if (!(sorted_keys[index] == expected_keys[index])
            || sorted_values[index].lo != expected_values[index]
            || sorted_values[index].hi != 0) {
            throw std::runtime_error("wide128 two-pass radix sort self-test failed");
        }
    }

    DeviceBuffer<WideKey> unique_keys(tracker, item_count, "allocate self-test unique keys");
    DeviceBuffer<uint64_t> run_lengths(tracker, item_count, "allocate self-test run lengths");
    DeviceBuffer<int> run_count(tracker, 1, "allocate self-test run count");
    size_t rle_temp_bytes = 0;
    check_cuda(
        cub::DeviceRunLengthEncode::Encode(
            nullptr,
            rle_temp_bytes,
            keys.get(),
            unique_keys.get(),
            run_lengths.get(),
            run_count.get(),
            static_cast<int>(item_count)),
        "query self-test run-length storage");
    DeviceBuffer<unsigned char> rle_temp(
        tracker, rle_temp_bytes, "allocate self-test run-length storage");
    check_cuda(
        cub::DeviceRunLengthEncode::Encode(
            rle_temp.get(),
            rle_temp_bytes,
            keys.get(),
            unique_keys.get(),
            run_lengths.get(),
            run_count.get(),
            static_cast<int>(item_count)),
        "run self-test run-length encode");
    int host_run_count = 0;
    uint64_t host_run_lengths[item_count]{};
    check_cuda(
        cudaMemcpy(&host_run_count, run_count.get(), sizeof(int), cudaMemcpyDeviceToHost),
        "copy self-test run count");
    check_cuda(
        cudaMemcpy(
            host_run_lengths,
            run_lengths.get(),
            item_count * sizeof(uint64_t),
            cudaMemcpyDeviceToHost),
        "copy self-test run lengths");
    if (host_run_count != 3 || host_run_lengths[0] != 1 || host_run_lengths[1] != 2
        || host_run_lengths[2] != 1) {
        throw std::runtime_error("wide128 run-length equality self-test failed");
    }

    DeviceBuffer<int> flags(tracker, 3, "allocate arithmetic self-test flags");
    DeviceBuffer<U128> carry_result(tracker, 1, "allocate arithmetic self-test result");
    arithmetic_self_test_kernel<<<1, 1>>>(flags.get(), carry_result.get());
    verify_kernel("launch arithmetic self-test kernel");
    int host_flags[3]{};
    U128 host_carry{};
    check_cuda(
        cudaMemcpy(host_flags, flags.get(), sizeof(host_flags), cudaMemcpyDeviceToHost),
        "copy arithmetic self-test flags");
    check_cuda(
        cudaMemcpy(&host_carry, carry_result.get(), sizeof(host_carry), cudaMemcpyDeviceToHost),
        "copy arithmetic self-test carry");
    if (host_flags[0] != 1 || host_flags[1] != 0 || host_flags[2] != 1
        || host_carry.lo != 0 || host_carry.hi != 1) {
        throw std::runtime_error("checked integer arithmetic self-test failed");
    }
}

} // namespace

extern "C" int nq_gpu_probe(uint32_t device_id, NqGpuDeviceInfo* info) {
    if (info == nullptr) {
        return 1;
    }
    std::memset(info, 0, sizeof(*info));
    info->device_id = device_id;
    try {
        check_cuda(cudaSetDevice(static_cast<int>(device_id)), "cudaSetDevice");
        cudaDeviceProp properties{};
        check_cuda(
            cudaGetDeviceProperties(&properties, static_cast<int>(device_id)),
            "cudaGetDeviceProperties");
        int driver_version = 0;
        int runtime_version = 0;
        check_cuda(cudaDriverGetVersion(&driver_version), "cudaDriverGetVersion");
        check_cuda(cudaRuntimeGetVersion(&runtime_version), "cudaRuntimeGetVersion");
        info->compute_major = static_cast<uint32_t>(properties.major);
        info->compute_minor = static_cast<uint32_t>(properties.minor);
        info->cuda_driver_version = static_cast<uint32_t>(driver_version);
        info->cuda_runtime_version = static_cast<uint32_t>(runtime_version);
        info->multiprocessor_count = static_cast<uint32_t>(properties.multiProcessorCount);
        info->total_global_memory = static_cast<uint64_t>(properties.totalGlobalMem);
        copy_string(info->device_name, sizeof(info->device_name), properties.name);
        return 0;
    } catch (const std::exception& error) {
        copy_string(info->error, sizeof(info->error), error.what());
        return 1;
    }
}

extern "C" int nq_gpu_self_test(
    uint32_t device_id, unsigned char* error, size_t error_capacity) {
    if (error == nullptr || error_capacity == 0) {
        return 1;
    }
    error[0] = 0;
    try {
        run_device_self_test(device_id);
        return 0;
    } catch (const std::exception& exception) {
        copy_string(error, error_capacity, exception.what());
        return 1;
    }
}

extern "C" int nq_gpu_contract(
    uint32_t n,
    const NqGpuTransitionDescriptor* descriptor,
    const NqGpuOptions* options,
    NqGpuRunResult* result,
    NqGpuLayerMetric* layers,
    size_t layer_capacity) {
    if (result == nullptr) {
        return 1;
    }
    std::memset(result, 0, sizeof(*result));
    result->n = n;
    try {
        if (descriptor == nullptr || options == nullptr) {
            throw std::invalid_argument("null GPU descriptor or options");
        }
        if (layer_capacity < n || (n > 0 && layers == nullptr)) {
            throw std::invalid_argument("GPU layer output capacity is smaller than N");
        }
        if (n > 42) {
            throw std::invalid_argument("wide128 packed boundaries support N <= 42");
        }
        if (options->memory_limit_percent == 0 || options->memory_limit_percent > 95) {
            throw std::invalid_argument("GPU memory limit percent must be in 1..=95");
        }
        if (options->scheme != NQ_GPU_SCHEME_COMPACT64
            && options->scheme != NQ_GPU_SCHEME_WIDE128) {
            throw std::invalid_argument("unknown GPU arithmetic scheme");
        }
        if (options->scheme == NQ_GPU_SCHEME_COMPACT64 && n > 20) {
            throw std::invalid_argument("compact64 is exact only for N <= 20");
        }
        validate_descriptor(*descriptor);
        uint64_t total_memory = 0;
        populate_device_fields(options->device_id, *result, total_memory);
        result->scheme = options->scheme;
        result->tensor_entries_examined = descriptor->tensor_entries_examined;
        result->tensor_entries_matched = descriptor->tensor_entries_matched;
        result->layer_count = n;
        MemoryTracker tracker(total_memory, options->memory_limit_percent);
        const auto start = std::chrono::steady_clock::now();
        U128 count{};
        if (n == 0) {
            count = U128{1, 0};
            result->peak_states = 1;
        } else if (options->scheme == NQ_GPU_SCHEME_COMPACT64) {
            count = run_contraction<uint64_t, uint64_t>(
                n, *descriptor, tracker, *result, layers);
        } else {
            count = run_contraction<WideKey, U128>(n, *descriptor, tracker, *result, layers);
        }
        const auto stop = std::chrono::steady_clock::now();
        result->host_elapsed_ns = static_cast<uint64_t>(
            std::chrono::duration_cast<std::chrono::nanoseconds>(stop - start).count());
        result->count_lo = count.lo;
        result->count_hi = count.hi;
        result->peak_device_bytes = tracker.peak_bytes();
        return 0;
    } catch (const std::exception& error) {
        copy_string(result->error, sizeof(result->error), error.what());
        return 1;
    }
}
