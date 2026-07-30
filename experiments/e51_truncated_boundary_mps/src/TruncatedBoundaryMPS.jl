module TruncatedBoundaryMPS

using LinearAlgebra

export BEntry,
    CEntry,
    ContractResult,
    LayerMetric,
    TruncationStats,
    VirtualLegs,
    V0,
    V1,
    V2,
    contract_truncated,
    known_count,
    line_boundary_weight,
    peak_rss_bytes,
    site_tensor_b,
    site_tensor_c

const V0 = (1.0, 0.0)
const V1 = (0.0, 1.0)
const V2 = (1.0, 1.0)

"""The eight binary virtual legs of the Sec. VI rank-9 site tensor B."""
struct VirtualLegs
    column_in::UInt8
    column_out::UInt8
    row_in::UInt8
    row_out::UInt8
    diag_dr_in::UInt8
    diag_dr_out::UInt8
    diag_dl_in::UInt8
    diag_dl_out::UInt8
end

struct BEntry
    alpha::UInt8
    legs::VirtualLegs
    value::Float64
end

struct CEntry
    legs::VirtualLegs
    value::Float64
end

"""
Construct Eq. (16) explicitly: sixteen alpha=0 pass-through entries and the
single alpha=1 entry that changes all four directed signals from zero to one.
"""
function site_tensor_b()
    entries = BEntry[]
    sizehint!(entries, 17)
    for signals in UInt8(0):UInt8(15)
        column = signals & UInt8(1)
        row = (signals >> 1) & UInt8(1)
        diag_dr = (signals >> 2) & UInt8(1)
        diag_dl = (signals >> 3) & UInt8(1)
        push!(entries, BEntry(
            UInt8(0),
            VirtualLegs(
                column,
                column,
                row,
                row,
                diag_dr,
                diag_dr,
                diag_dl,
                diag_dl,
            ),
            1.0,
        ))
    end
    push!(entries, BEntry(
        UInt8(1),
        VirtualLegs(0, 1, 0, 1, 0, 1, 0, 1),
        1.0,
    ))
    return entries
end

function legs_key(legs::VirtualLegs)
    return UInt16(legs.column_in) |
           (UInt16(legs.column_out) << 1) |
           (UInt16(legs.row_in) << 2) |
           (UInt16(legs.row_out) << 3) |
           (UInt16(legs.diag_dr_in) << 4) |
           (UInt16(legs.diag_dr_out) << 5) |
           (UInt16(legs.diag_dl_in) << 6) |
           (UInt16(legs.diag_dl_out) << 7)
end

"""Construct the rank-8 counting tensor C by explicitly summing B over alpha."""
function site_tensor_c(entries_b::Vector{BEntry} = site_tensor_b())
    accumulated = Dict{VirtualLegs,Float64}()
    for entry in entries_b
        accumulated[entry.legs] = get(accumulated, entry.legs, 0.0) + entry.value
    end
    entries = [CEntry(legs, value) for (legs, value) in accumulated]
    sort!(entries; by = entry -> legs_key(entry.legs))
    return entries
end

function channel_pair(legs::VirtualLegs, family::Symbol)
    family === :column && return (legs.column_in, legs.column_out)
    family === :row && return (legs.row_in, legs.row_out)
    family === :diag_dr && return (legs.diag_dr_in, legs.diag_dr_out)
    family === :diag_dl && return (legs.diag_dl_in, legs.diag_dl_out)
    throw(ArgumentError("unknown constraint family: $family"))
end

"""
Contract one directed constraint line with v0 at its start and v1 or v2 at its
end. The one-channel truth table is projected from the explicit B entries.
This helper is for boundary validation, not for the board contraction.
"""
function line_boundary_weight(
    occupations::AbstractVector{<:Integer},
    endpoint::Symbol;
    family::Symbol = :row,
)
    endpoint_vector = endpoint === :v1 ? V1 : endpoint === :v2 ? V2 :
                      throw(ArgumentError("endpoint must be :v1 or :v2"))
    entries_b = site_tensor_b()
    state = collect(V0)
    for alpha in occupations
        alpha in (0, 1) || throw(ArgumentError("occupations must be binary"))
        transitions = Set{Tuple{UInt8,UInt8}}()
        for entry in entries_b
            entry.alpha == alpha || continue
            push!(transitions, channel_pair(entry.legs, family))
        end
        next_state = zeros(Float64, 2)
        for (incoming, outgoing) in transitions
            next_state[Int(outgoing) + 1] += state[Int(incoming) + 1]
        end
        state = next_state
    end
    return state[1] * endpoint_vector[1] + state[2] * endpoint_vector[2]
end

const KNOWN_COUNTS = Int128[
    1,
    1,
    0,
    0,
    2,
    10,
    4,
    40,
    92,
    352,
    724,
    2_680,
    14_200,
    73_712,
    365_596,
    2_279_184,
    14_772_512,
    95_815_104,
    666_090_624,
    4_968_057_848,
    39_029_188_884,
    314_666_222_712,
    2_691_008_701_644,
    24_233_937_684_440,
    227_514_171_973_736,
    2_207_893_435_808_352,
    22_317_699_616_364_044,
    234_907_967_154_122_528,
]

known_count(n::Integer) = 0 <= n < length(KNOWN_COUNTS) ? KNOWN_COUNTS[n + 1] : nothing

Base.@kwdef mutable struct TruncationStats
    svd_calls::Int = 0
    truncated_svd_calls::Int = 0
    peak_pretruncate_rank::Int = 1
    peak_retained_bond::Int = 1
    peak_working_bond::Int = 1
    peak_mps_elements::Int = 0
    max_discarded_fraction::Float64 = 0.0
    sum_discarded_fraction::Float64 = 0.0
    svd_elapsed_s::Float64 = 0.0
    tensor_entries_examined::Int128 = 0
    tensor_entries_accepted::Int128 = 0
end

struct LayerMetric
    row::Int
    max_bond_after_mpo::Int
    max_bond_after_shift::Int
    truncated_svd_calls::Int
    max_discarded_fraction::Float64
end

struct ContractResult
    n::Int
    chi::Int
    estimate::Float64
    elapsed_s::Float64
    peak_rss_bytes::Int
    stats::TruncationStats
    layers::Vector{LayerMetric}
end

const Tensor3 = Array{Float64,3}
const MPS = Vector{Tensor3}

function max_bond(mps::MPS)
    isempty(mps) && return 1
    return maximum(max(size(site, 1), size(site, 3)) for site in mps)
end

function observe_mps!(stats::TruncationStats, mps::MPS; retained_checkpoint::Bool = false)
    working_bond = max_bond(mps)
    stats.peak_working_bond = max(stats.peak_working_bond, working_bond)
    if retained_checkpoint
        stats.peak_retained_bond = max(stats.peak_retained_bond, working_bond)
    end
    stats.peak_mps_elements = max(stats.peak_mps_elements, sum(length, mps))
    return nothing
end

function checked_chi(chi::Integer)
    chi >= 0 || throw(ArgumentError("chi must be nonnegative; zero means no user bond cap"))
    return Int(chi)
end

"""
Compute a thin SVD and retain at most chi numerical singular directions. A
zero chi means no user cap; numerical null directions below the standard
LAPACK rank tolerance are still removed. The discarded fractions are local
Frobenius diagnostics and are not rigorous global error bounds.
"""
function truncated_svd(matrix::Matrix{Float64}, chi::Int, stats::TruncationStats)
    started = time_ns()
    factors = svd(matrix; full = false)
    stats.svd_elapsed_s += (time_ns() - started) / 1.0e9
    stats.svd_calls += 1

    singular_values = factors.S
    leading = isempty(singular_values) ? 0.0 : singular_values[1]
    tolerance = eps(Float64) * max(size(matrix)...) * leading
    numerical_rank = count(value -> value > tolerance, singular_values)
    stats.peak_pretruncate_rank = max(stats.peak_pretruncate_rank, numerical_rank)

    retained = max(numerical_rank, 1)
    chi > 0 && (retained = min(retained, chi))
    retained = min(retained, length(singular_values))

    total_norm2 = sum(abs2, singular_values)
    discarded_norm2 = retained < length(singular_values) ?
                      sum(abs2, @view(singular_values[(retained + 1):end])) : 0.0
    discarded_fraction = total_norm2 == 0.0 ? 0.0 : discarded_norm2 / total_norm2
    stats.max_discarded_fraction = max(stats.max_discarded_fraction, discarded_fraction)
    stats.sum_discarded_fraction += discarded_fraction
    if retained < numerical_rank
        stats.truncated_svd_calls += 1
    end
    left = Matrix(@view factors.U[:, 1:retained])
    values = Vector(@view singular_values[1:retained])
    right = Matrix(@view factors.Vt[1:retained, :])
    return left, values, right
end

function left_canonicalize_through!(mps::MPS, final_index::Int)
    final_index <= 0 && return mps
    for index in 1:min(final_index, length(mps) - 1)
        site = mps[index]
        left, physical, right = size(site)
        matrix = reshape(site, left * physical, right)
        factors = qr(matrix)
        new_right = min(size(matrix)...)
        q = Matrix(factors.Q[:, 1:new_right])
        r = Matrix(factors.R[1:new_right, :])
        mps[index] = reshape(q, left, physical, new_right)

        next_site = mps[index + 1]
        size(next_site, 1) == right || error("MPS bond mismatch during QR sweep")
        next_matrix = reshape(next_site, right, :)
        mps[index + 1] = reshape(
            r * next_matrix,
            new_right,
            size(next_site, 2),
            size(next_site, 3),
        )
    end
    return mps
end

function right_canonicalize_from!(mps::MPS, first_index::Int)
    first_index > length(mps) && return mps
    for index in length(mps):-1:max(first_index, 2)
        site = mps[index]
        left, physical, right = size(site)
        matrix = reshape(site, left, physical * right)
        factors = qr(Matrix(transpose(matrix)))
        new_left = min(size(matrix)...)
        q = Matrix(factors.Q[:, 1:new_left])
        r = Matrix(factors.R[1:new_left, :])
        mps[index] = reshape(Matrix(transpose(q)), new_left, physical, right)

        previous = mps[index - 1]
        size(previous, 3) == left || error("MPS bond mismatch during right QR sweep")
        previous_matrix = reshape(previous, :, left)
        transfer = Matrix(transpose(r))
        mps[index - 1] = reshape(
            previous_matrix * transfer,
            size(previous, 1),
            size(previous, 2),
            new_left,
        )
    end
    return mps
end

"""Place the mixed-canonical orthogonality center on a two-site bond."""
function canonicalize_around_bond!(mps::MPS, bond::Int)
    1 <= bond < length(mps) || throw(BoundsError(mps, bond))
    left_canonicalize_through!(mps, bond - 1)
    right_canonicalize_from!(mps, bond + 2)
    return mps
end

function compress_mps!(mps::MPS, chi::Int, stats::TruncationStats)
    length(mps) <= 1 && return mps
    left_canonicalize_through!(mps, length(mps) - 1)
    for index in length(mps):-1:2
        site = mps[index]
        left, physical, right = size(site)
        matrix = reshape(site, left, physical * right)
        u, singular_values, vt = truncated_svd(matrix, chi, stats)
        retained = length(singular_values)
        mps[index] = reshape(vt, retained, physical, right)

        transfer = u .* reshape(singular_values, 1, retained)
        previous = mps[index - 1]
        size(previous, 3) == left || error("MPS bond mismatch during SVD sweep")
        previous_matrix = reshape(previous, :, left)
        mps[index - 1] = reshape(
            previous_matrix * transfer,
            size(previous, 1),
            size(previous, 2),
            retained,
        )
    end
    observe_mps!(stats, mps; retained_checkpoint = true)
    return mps
end

function apply_row_mpo(
    mps::MPS,
    n::Int,
    entries_c::Vector{CEntry},
    chi::Int,
    stats::TruncationStats,
)
    length(mps) == n || error("row MPO expects N MPS sites")
    all(size(site, 2) == 8 for site in mps) || error("row MPO expects physical dimension 8")
    output = MPS()
    sizehint!(output, n)

    for column in 1:n
        input = mps[column]
        input_left, _, input_right = size(input)
        left_states = column == 1 ? (UInt8(0),) : (UInt8(0), UInt8(1))
        right_states = column == n ? (UInt8(1),) : (UInt8(0), UInt8(1))
        site = zeros(Float64, input_left * length(left_states), 8, input_right * length(right_states))

        for entry in entries_c
            stats.tensor_entries_examined += 1
            left_position = findfirst(==(entry.legs.row_in), left_states)
            isnothing(left_position) && continue
            right_position = findfirst(==(entry.legs.row_out), right_states)
            isnothing(right_position) && continue
            stats.tensor_entries_accepted += 1

            physical_in = Int(entry.legs.column_in) |
                          (Int(entry.legs.diag_dr_in) << 1) |
                          (Int(entry.legs.diag_dl_in) << 2)
            physical_out = Int(entry.legs.column_out) |
                           (Int(entry.legs.diag_dr_out) << 1) |
                           (Int(entry.legs.diag_dl_out) << 2)
            left_offset = (left_position - 1) * input_left
            right_offset = (right_position - 1) * input_right
            @views site[
                (left_offset + 1):(left_offset + input_left),
                physical_out + 1,
                (right_offset + 1):(right_offset + input_right),
            ] .+= entry.value .* input[:, physical_in + 1, :]
        end
        push!(output, site)
    end
    compress_mps!(output, chi, stats)
    return output
end

function split_site(site::Tensor3, stats::TruncationStats)
    left, physical, right = size(site)
    physical == 8 || error("split_site expects physical dimension 8")

    first_matrix = reshape(site, left * 2, 4 * right)
    # The three qubits are an exact rewriting of one physical-dimension-eight
    # site. Do not impose chi on these artificial intra-site bonds; truncation
    # is performed only at globally canonical MPS cuts.
    first, first_values, first_right = truncated_svd(first_matrix, 0, stats)
    rank_one = length(first_values)
    column_site = reshape(first, left, 2, rank_one)
    remainder = (reshape(first_values, rank_one, 1) .* first_right)
    remainder = reshape(remainder, rank_one, 4, right)

    second_matrix = reshape(remainder, rank_one * 2, 2 * right)
    second, second_values, second_right = truncated_svd(second_matrix, 0, stats)
    rank_two = length(second_values)
    diag_right_site = reshape(second, rank_one, 2, rank_two)
    diag_left_site = reshape(
        reshape(second_values, rank_two, 1) .* second_right,
        rank_two,
        2,
        right,
    )
    return column_site, diag_right_site, diag_left_site
end

struct WireLabel
    family::Symbol
    column::Int
end

function split_all(mps::MPS, n::Int, stats::TruncationStats)
    qubits = MPS()
    labels = WireLabel[]
    sizehint!(qubits, 3 * n)
    sizehint!(labels, 3 * n)
    for column in 1:n
        append!(qubits, split_site(mps[column], stats))
        append!(labels, (
            WireLabel(:column, column),
            WireLabel(:diag_right, column),
            WireLabel(:diag_left, column),
        ))
    end
    observe_mps!(stats, qubits)
    return qubits, labels
end

function swap_adjacent!(mps::MPS, left_index::Int, chi::Int, stats::TruncationStats)
    # A two-site SVD is a Schmidt truncation only when the left and right
    # environments are orthonormal. This mixed-canonical placement removes the
    # gauge dependence of the original E51 pilot.
    canonicalize_around_bond!(mps, left_index)
    left_site = mps[left_index]
    right_site = mps[left_index + 1]
    left_outer, left_physical, middle = size(left_site)
    right_middle, right_physical, right_outer = size(right_site)
    middle == right_middle || error("bond mismatch in adjacent SWAP")
    left_physical == 2 && right_physical == 2 || error("SWAP expects qubit sites")

    contracted = reshape(left_site, left_outer * 2, middle) *
                 reshape(right_site, middle, 2 * right_outer)
    contracted = reshape(contracted, left_outer, 2, 2, right_outer)
    swapped = permutedims(contracted, (1, 3, 2, 4))
    matrix = reshape(swapped, left_outer * 2, 2 * right_outer)
    u, singular_values, vt = truncated_svd(matrix, chi, stats)
    retained = length(singular_values)
    stats.peak_retained_bond = max(stats.peak_retained_bond, retained)
    mps[left_index] = reshape(u, left_outer, 2, retained)
    mps[left_index + 1] = reshape(
        reshape(singular_values, retained, 1) .* vt,
        retained,
        2,
        right_outer,
    )
    observe_mps!(stats, mps)
    return nothing
end

function remove_with_vector!(mps::MPS, index::Int, vector::Tuple{Float64,Float64})
    removed = mps[index]
    left, physical, right = size(removed)
    physical == 2 || error("boundary removal expects a qubit site")
    transfer = vector[1] .* removed[:, 1, :] .+ vector[2] .* removed[:, 2, :]
    transfer = reshape(transfer, left, right)

    if index < length(mps)
        next_site = mps[index + 1]
        size(next_site, 1) == right || error("right bond mismatch during boundary removal")
        merged = transfer * reshape(next_site, right, :)
        mps[index + 1] = reshape(merged, left, size(next_site, 2), size(next_site, 3))
        deleteat!(mps, index)
    elseif index > 1
        previous = mps[index - 1]
        size(previous, 3) == left || error("left bond mismatch during boundary removal")
        merged = reshape(previous, :, left) * transfer
        mps[index - 1] = reshape(merged, size(previous, 1), size(previous, 2), right)
        deleteat!(mps, index)
    else
        error("cannot remove the only MPS site")
    end
    return nothing
end

function append_fixed!(mps::MPS, vector::Tuple{Float64,Float64})
    isempty(mps) && error("cannot append to an empty MPS")
    size(mps[end], 3) == 1 || error("open-boundary MPS must end at bond one")
    site = zeros(Float64, 1, 2, 1)
    site[1, 1, 1] = vector[1]
    site[1, 2, 1] = vector[2]
    push!(mps, site)
    return nothing
end

function merge_adjacent(first::Tensor3, second::Tensor3)
    left, first_physical, middle = size(first)
    second_middle, second_physical, right = size(second)
    middle == second_middle || error("bond mismatch while grouping sites")
    matrix = reshape(first, left * first_physical, middle) * reshape(second, middle, :)
    return reshape(matrix, left, first_physical * second_physical, right)
end

function shift_diagonals(mps::MPS, n::Int, chi::Int, stats::TruncationStats)
    qubits, labels = split_all(mps, n, stats)
    removals = Int[
        something(findfirst(==(WireLabel(:diag_right, n)), labels)),
        something(findfirst(==(WireLabel(:diag_left, 1)), labels)),
    ]
    sort!(removals; rev = true)
    for index in removals
        remove_with_vector!(qubits, index, V2)
        deleteat!(labels, index)
    end

    append_fixed!(qubits, V0)
    push!(labels, WireLabel(:new_diag_right, 0))
    append_fixed!(qubits, V0)
    push!(labels, WireLabel(:new_diag_left, 0))

    target = WireLabel[]
    sizehint!(target, 3 * n)
    for column in 1:n
        push!(target, WireLabel(:column, column))
        push!(target, column == 1 ?
              WireLabel(:new_diag_right, 0) : WireLabel(:diag_right, column - 1))
        push!(target, column == n ?
              WireLabel(:new_diag_left, 0) : WireLabel(:diag_left, column + 1))
    end

    for target_index in eachindex(target)
        current = something(findfirst(==(target[target_index]), labels))
        while current > target_index
            swap_adjacent!(qubits, current - 1, chi, stats)
            labels[current - 1], labels[current] = labels[current], labels[current - 1]
            current -= 1
        end
    end
    labels == target || error("diagonal wire permutation failed")

    grouped = MPS()
    sizehint!(grouped, n)
    for column in 1:n
        first = qubits[3 * column - 2]
        second = qubits[3 * column - 1]
        third = qubits[3 * column]
        push!(grouped, merge_adjacent(merge_adjacent(first, second), third))
    end
    compress_mps!(grouped, chi, stats)
    return grouped
end

function final_boundary_contraction(mps::MPS)
    vector = [1.0]
    for site in mps
        length(vector) == size(site, 1) || error("final MPS left bond mismatch")
        size(site, 2) == 8 || error("final MPS physical dimension mismatch")
        next_vector = zeros(Float64, size(site, 3))
        for left in axes(site, 1), physical_zero_based in 0:7
            (physical_zero_based & 1) == 1 || continue # column v1
            amplitude = vector[left]
            amplitude == 0.0 && continue
            @views next_vector .+= amplitude .* site[left, physical_zero_based + 1, :]
        end
        vector = next_vector
    end
    length(vector) == 1 || error("final MPS right boundary is not scalar")
    return vector[1]
end

function peak_rss_bytes()
    Sys.islinux() || return 0
    try
        for line in eachline("/proc/self/status")
            startswith(line, "VmHWM:") || continue
            fields = split(line)
            return parse(Int, fields[2]) * 1024
        end
    catch
        return 0
    end
    return 0
end

"""
Contract the Sec. VI network as a floating-point boundary MPS.

`chi > 0` imposes the maximum retained MPS bond dimension. `chi == 0` removes
the user cap and exists only for small-N geometry validation. Both modes use
floating-point SVD and therefore neither is an exact integer algorithm.
"""
function contract_truncated(n::Integer, chi::Integer)
    n >= 0 || throw(ArgumentError("N must be nonnegative"))
    n = Int(n)
    chi = checked_chi(chi)
    stats = TruncationStats()
    layers = LayerMetric[]
    started = time_ns()

    if n == 0
        return ContractResult(0, chi, 1.0, 0.0, peak_rss_bytes(), stats, layers)
    end

    entries_b = site_tensor_b()
    entries_c = site_tensor_c(entries_b)
    length(entries_b) == 17 || error("Sec. VI B must have exactly 17 entries")
    length(entries_c) == 17 || error("Sec. VI C must have exactly 17 entries")

    boundary = MPS()
    for _ in 1:n
        site = zeros(Float64, 1, 8, 1)
        site[1, 1, 1] = 1.0 # column/diagonal incoming v0 signals
        push!(boundary, site)
    end
    observe_mps!(stats, boundary; retained_checkpoint = true)

    estimate = NaN
    for row in 1:n
        after_mpo = apply_row_mpo(boundary, n, entries_c, chi, stats)
        mpo_bond = max_bond(after_mpo)
        if row == n
            estimate = final_boundary_contraction(after_mpo)
            push!(layers, LayerMetric(
                row,
                mpo_bond,
                1,
                stats.truncated_svd_calls,
                stats.max_discarded_fraction,
            ))
        else
            boundary = shift_diagonals(after_mpo, n, chi, stats)
            push!(layers, LayerMetric(
                row,
                mpo_bond,
                max_bond(boundary),
                stats.truncated_svd_calls,
                stats.max_discarded_fraction,
            ))
        end
    end

    elapsed_s = (time_ns() - started) / 1.0e9
    return ContractResult(n, chi, estimate, elapsed_s, peak_rss_bytes(), stats, layers)
end

end # module
