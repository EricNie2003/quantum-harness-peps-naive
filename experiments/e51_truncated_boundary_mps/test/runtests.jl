using Test
using TruncatedBoundaryMPS

@testset "explicit Sec. VI tensors" begin
    tensor_b = site_tensor_b()
    tensor_c = site_tensor_c(tensor_b)
    @test length(tensor_b) == 17
    @test length(tensor_c) == 17

    empty_entries = filter(entry -> entry.alpha == 0, tensor_b)
    occupied_entries = filter(entry -> entry.alpha == 1, tensor_b)
    @test length(empty_entries) == 16
    @test length(occupied_entries) == 1

    signatures = Set{NTuple{4,UInt8}}()
    for entry in empty_entries
        legs = entry.legs
        @test legs.column_in == legs.column_out
        @test legs.row_in == legs.row_out
        @test legs.diag_dr_in == legs.diag_dr_out
        @test legs.diag_dl_in == legs.diag_dl_out
        push!(signatures, (
            legs.column_in,
            legs.row_in,
            legs.diag_dr_in,
            legs.diag_dl_in,
        ))
    end
    @test length(signatures) == 16

    occupied = only(occupied_entries)
    @test occupied.legs == VirtualLegs(0, 1, 0, 1, 0, 1, 0, 1)
    @test occupied.value == 1.0

    summed_b = Dict{VirtualLegs,Float64}()
    for entry in tensor_b
        summed_b[entry.legs] = get(summed_b, entry.legs, 0.0) + entry.value
    end
    @test Dict(entry.legs => entry.value for entry in tensor_c) == summed_b
end
@testset "v0, v1, and v2 line boundaries" begin
    for family in (:column, :row, :diag_dr, :diag_dl)
        for length in 0:5
            for bits in 0:(2^length - 1)
                occupations = [(bits >> offset) & 1 for offset in 0:(length - 1)]
                queens = sum(occupations)
                @test line_boundary_weight(occupations, :v1; family) == (queens == 1 ? 1.0 : 0.0)
                @test line_boundary_weight(occupations, :v2; family) == (queens <= 1 ? 1.0 : 0.0)
            end
        end
    end
end

@testset "uncapped floating boundary MPS geometry" begin
    for n in 0:7
        result = contract_truncated(n, 0)
        expected = Float64(known_count(n))
        @test isapprox(result.estimate, expected; rtol = 5e-10, atol = 5e-9)
        @test result.stats.truncated_svd_calls == 0
        @test result.stats.tensor_entries_examined == Int128(17 * n * n)
    end
end

@testset "finite bond cap is explicit and diagnostic" begin
    result = contract_truncated(7, 2)
    @test isfinite(result.estimate)
    @test result.stats.truncated_svd_calls > 0
    @test result.stats.peak_retained_bond <= 2
    @test result.stats.max_discarded_fraction >= 0.0
    @test result.stats.sum_discarded_fraction >= result.stats.max_discarded_fraction
end
