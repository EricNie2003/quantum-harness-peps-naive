using LinearAlgebra
using Printf
using TruncatedBoundaryMPS

function parse_integer(value::AbstractString, label::AbstractString)
    parsed = tryparse(Int, value)
    isnothing(parsed) && error("invalid $label: $value")
    return parsed
end

function usage()
    println(stderr, "usage: benchmark_point.jl N CHI REPEATS WARMUP [LAYERS_01]")
end

function csv_value(value::Union{Nothing,Int128})
    return isnothing(value) ? "" : string(value)
end

function median_value(values::AbstractVector{<:Real})
    isempty(values) && error("median requires at least one value")
    ordered = sort(collect(values))
    middle = length(ordered) ÷ 2
    return isodd(length(ordered)) ? ordered[middle + 1] :
           (ordered[middle] + ordered[middle + 1]) / 2
end

function run()
    length(ARGS) in (4, 5) || (usage(); error("wrong number of arguments"))
    n = parse_integer(ARGS[1], "N")
    chi = parse_integer(ARGS[2], "CHI")
    repeats = parse_integer(ARGS[3], "REPEATS")
    warmup = parse_integer(ARGS[4], "WARMUP")
    show_layers = length(ARGS) == 5 ? parse_integer(ARGS[5], "LAYERS_01") == 1 : false
    n >= 0 || error("N must be nonnegative")
    chi >= 0 || error("CHI must be nonnegative")
    repeats > 0 || error("REPEATS must be positive")
    warmup >= 0 || error("WARMUP must be nonnegative")

    for _ in 1:warmup
        contract_truncated(n, chi)
    end

    results = [contract_truncated(n, chi) for _ in 1:repeats]
    elapsed = [result.elapsed_s for result in results]
    estimates = [result.estimate for result in results]
    order = sortperm(elapsed)
    selected = results[order[cld(length(order), 2)]]
    estimate = median_value(estimates)
    exact = known_count(n)
    absolute_error = isnothing(exact) ? NaN : abs(estimate - Float64(exact))
    relative_error = isnothing(exact) || exact == 0 ? NaN : absolute_error / Float64(exact)
    tolerance = isnothing(exact) ? NaN : max(5e-9, 5e-10 * abs(Float64(exact)))
    geometry_check = selected.stats.truncated_svd_calls == 0 &&
                     !isnothing(exact) && absolute_error <= tolerance
    status = geometry_check ? "floating_uncapped_check_pass" : "approximate_diagnostic"

    println(
        "algorithm_class,N,chi,status,estimate,exact_count,absolute_error,relative_error," *
        "median_elapsed_s,min_elapsed_s,max_elapsed_s,estimate_min,estimate_max,repeats,warmup," *
        "peak_rss_bytes,peak_sparse_support,peak_dense_mps_elements,peak_retained_bond," *
        "peak_working_bond,peak_pretruncate_rank,svd_calls,truncated_svd_calls,max_discarded_fraction," *
        "sum_discarded_fraction,svd_elapsed_s,tensor_entries_examined,tensor_entries_accepted," *
        "blas_threads,julia_threads,truncation_occurred",
    )
    @printf(
        "truncated_boundary_mps_float64,%d,%d,%s,%.17g,%s,%.17g,%.17g,%.9f,%.9f,%.9f,%.17g,%.17g,%d,%d,%d,NA,%d,%d,%d,%d,%d,%d,%.17g,%.17g,%.9f,%s,%s,%d,%d,%s\n",
        n,
        chi,
        status,
        estimate,
        csv_value(exact),
        absolute_error,
        relative_error,
        median_value(elapsed),
        minimum(elapsed),
        maximum(elapsed),
        minimum(estimates),
        maximum(estimates),
        repeats,
        warmup,
        maximum(result.peak_rss_bytes for result in results),
        selected.stats.peak_mps_elements,
        selected.stats.peak_retained_bond,
        selected.stats.peak_working_bond,
        selected.stats.peak_pretruncate_rank,
        selected.stats.svd_calls,
        selected.stats.truncated_svd_calls,
        selected.stats.max_discarded_fraction,
        selected.stats.sum_discarded_fraction,
        selected.stats.svd_elapsed_s,
        string(selected.stats.tensor_entries_examined),
        string(selected.stats.tensor_entries_accepted),
        BLAS.get_num_threads(),
        Threads.nthreads(),
        selected.stats.truncated_svd_calls > 0,
    )

    if show_layers
        println(stderr, "N,row,max_bond_after_mpo,max_bond_after_shift,cumulative_truncated_svd_calls,max_discarded_fraction")
        for layer in selected.layers
            @printf(
                stderr,
                "%d,%d,%d,%d,%d,%.17g\n",
                n,
                layer.row,
                layer.max_bond_after_mpo,
                layer.max_bond_after_shift,
                layer.truncated_svd_calls,
                layer.max_discarded_fraction,
            )
        end
    end
end

try
    run()
catch exception
    showerror(stderr, exception, catch_backtrace())
    println(stderr)
    exit(1)
end
