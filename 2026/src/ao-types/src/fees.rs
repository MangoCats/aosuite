use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Zero};

/// Compute recording fee in shares.
///
/// `fee_shares = ceil(data_bytes * fee_rate_num * shares_out / fee_rate_den)`
///
/// Uses arbitrary-precision integers. Multiply before dividing.
/// ceil(a/b) for positive a,b = (a + b - 1) / b (integer division, truncating).
pub fn recording_fee(
    data_bytes: u64,
    fee_rate_num: &BigInt,
    fee_rate_den: &BigInt,
    shares_out: &BigInt,
) -> BigInt {
    let numerator = BigInt::from(data_bytes) * fee_rate_num * shares_out;
    ceil_div(&numerator, fee_rate_den)
}

/// Compute share reward for the recorder.
///
/// `reward_shares = ceil(giver_total * reward_rate_num / reward_rate_den)`
///
/// Returns zero when the reward rate numerator is zero.
pub fn share_reward(
    giver_total: &BigInt,
    reward_rate_num: &BigInt,
    reward_rate_den: &BigInt,
) -> BigInt {
    if reward_rate_num.is_zero() {
        return BigInt::ZERO;
    }
    let numerator = giver_total * reward_rate_num;
    ceil_div(&numerator, reward_rate_den)
}

/// Ceiling division for positive integers: ceil(a / b) = (a + b - 1) / b.
fn ceil_div(a: &BigInt, b: &BigInt) -> BigInt {
    debug_assert!(*a >= BigInt::ZERO, "ceil_div requires non-negative numerator");
    debug_assert!(*b > BigInt::ZERO, "ceil_div requires positive denominator");
    let (q, r) = a.div_rem(b);
    if r > BigInt::ZERO {
        q + BigInt::one()
    } else {
        q
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bi(s: &str) -> BigInt {
        s.parse().unwrap()
    }

    #[test]
    fn test_fee_conformance() {
        let vectors: &[(u64, &str, &str, &str, &str, &str)] = &[
            // (data_bytes, fee_num, fee_den, shares_out, expected_numerator, expected_fee)
            (
                500, "1", "1000000",
                "77371252455336267181195264",
                "38685626227668133590597632000",
                "38685626227668133590598",
            ),
            (
                2000, "1", "1000000",
                "77371252455336267181195264",
                "154742504910672534362390528000",
                "154742504910672534362391",
            ),
            (
                500, "1", "1000000",
                "38685626227668133590597632",
                "19342813113834066795298816000",
                "19342813113834066795299",
            ),
            (
                214, "1", "1000000",
                "77371252455336267181195264",
                "16557448025441961176775786496",
                "16557448025441961176776",
            ),
            (
                500, "3", "10000000000",
                "77371252455336267181195264",
                "116056878683004400771792896000",
                "11605687868300440078",
            ),
        ];

        for (i, &(data_bytes, num, den, shares, exp_numerator, exp_fee)) in
            vectors.iter().enumerate()
        {
            let fee_num = bi(num);
            let fee_den = bi(den);
            let shares_out = bi(shares);

            // Verify numerator
            let numerator = BigInt::from(data_bytes) * &fee_num * &shares_out;
            assert_eq!(
                numerator.to_string(),
                exp_numerator,
                "vector {}: numerator mismatch",
                i
            );

            let fee = recording_fee(data_bytes, &fee_num, &fee_den, &shares_out);
            assert_eq!(
                fee.to_string(),
                exp_fee,
                "vector {}: fee mismatch",
                i
            );
        }
    }

    #[test]
    fn test_ceil_div_exact() {
        // 10 / 5 = 2 exactly
        assert_eq!(recording_fee(10, &bi("1"), &bi("5"), &bi("1")), bi("2"));
    }

    #[test]
    fn test_ceil_div_rounds_up() {
        // 11 / 5 = 2.2 → ceil = 3
        assert_eq!(recording_fee(11, &bi("1"), &bi("5"), &bi("1")), bi("3"));
    }

    #[test]
    fn test_share_reward_zero_rate() {
        assert_eq!(share_reward(&bi("1000"), &bi("0"), &bi("1")), bi("0"));
    }

    #[test]
    fn test_share_reward_exact() {
        // 1000 * 1/100 = 10
        assert_eq!(share_reward(&bi("1000"), &bi("1"), &bi("100")), bi("10"));
    }

    #[test]
    fn test_share_reward_rounds_up() {
        // 1001 * 1/100 = 10.01 → ceil = 11
        assert_eq!(share_reward(&bi("1001"), &bi("1"), &bi("100")), bi("11"));
    }

    #[test]
    fn test_share_reward_large_amounts() {
        // Large giver total: 77371252455336267181195264 * 1/1000000
        let giver = bi("77371252455336267181195264");
        let reward = share_reward(&giver, &bi("1"), &bi("1000000"));
        assert_eq!(reward, bi("77371252455336267182"));
    }
}
