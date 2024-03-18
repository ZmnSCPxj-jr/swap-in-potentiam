/*
Basic operations on scalars.
*/
use secp256k1::Scalar;
use secp256k1::SecretKey;

fn scalar_to_sk(a: &Scalar) -> SecretKey {
	SecretKey::from_slice(
		&a.clone().to_be_bytes()
	).expect("only use this if you already know non-0")
}

pub(crate)
fn scalar_plus(a: &Scalar, b: &Scalar) -> Scalar {
	if a == &Scalar::ZERO {
		return b.clone();
	}
	let sk_a = scalar_to_sk(a);
	let sum = sk_a.add_tweak(b);
	match sum {
		Err(_) => Scalar::ZERO,
		Ok(sum) => Scalar::from(sum)
	}
}

pub(crate)
fn scalar_negate(a: &Scalar) -> Scalar {
	if a == &Scalar::ZERO {
		return a.clone();
	}
	let sk_a = scalar_to_sk(a);
	Scalar::from(sk_a.negate())
}

mod test {
	use hex;
	use super::*;

	fn scalar(a: &str) -> Scalar {
		let raw_buf = hex::decode(a)
		.expect("Test input must be hex");
		let mut buf: [u8; 32] = [0; 32];
		buf.clone_from_slice(&raw_buf);
		Scalar::from_be_bytes(buf)
		.expect("Test input must be valid scalar")
	}

	fn check_plus(a: &str, b: &str, a_plus_b: &str) {
		let a = scalar(a);
		let b = scalar(b);
		let a_plus_b = scalar(a_plus_b);
		assert_eq!( scalar_plus(&a, &b)
			  , a_plus_b
			  );
	}

	#[test]
	fn test_scalar_plus() {
		/* 0 + 0 */
		assert_eq!( scalar_plus(&Scalar::ZERO, &Scalar::ZERO)
			  , Scalar::ZERO
			  );
		/* 0 + 1 */
		assert_eq!( scalar_plus(&Scalar::ZERO, &Scalar::ONE)
			  , Scalar::ONE
			  );
		/* 1 + 0 */
		assert_eq!( scalar_plus(&Scalar::ONE, &Scalar::ZERO)
			  , Scalar::ONE
			  );
		/* 1 + -1 */
		assert_eq!( scalar_plus(&Scalar::ONE, &Scalar::MAX)
			  , Scalar::ZERO
			  );
		/* -1 + 1 */
		assert_eq!( scalar_plus(&Scalar::MAX, &Scalar::ONE)
			  , Scalar::ZERO
			  );

		/* 1 + 2 = 3 */
		check_plus( "0000000000000000000000000000000000000000000000000000000000000001"
			  , "0000000000000000000000000000000000000000000000000000000000000002"
			  , "0000000000000000000000000000000000000000000000000000000000000003"
			  );

		/* 511 + 1 = 512 */
		check_plus( "00000000000000000000000000000000000000000000000000000000000001FF"
			  , "0000000000000000000000000000000000000000000000000000000000000001"
			  , "0000000000000000000000000000000000000000000000000000000000000200"
			  );
		/* 1 + 511 = 512 */
		check_plus( "0000000000000000000000000000000000000000000000000000000000000001"
			  , "00000000000000000000000000000000000000000000000000000000000001FF"
			  , "0000000000000000000000000000000000000000000000000000000000000200"
			  );

	}

	#[test]
	fn test_scalar_negate() {
		/* 1 + -(1) = 0 */
		assert_eq!( scalar_plus(&Scalar::ONE, &scalar_negate(&Scalar::ONE))
			  , Scalar::ZERO
			  );

		/* -(-1) = 1 */
		assert_eq!( scalar_negate(&Scalar::MAX)
			  , Scalar::ONE
			  );

		/* -(1) = -1 */
		assert_eq!( scalar_negate(&Scalar::ONE)
			  , Scalar::MAX
			  );

		/* -(0) = 0 */
		assert_eq!( scalar_negate(&Scalar::ZERO)
			  , Scalar::ZERO
			  );

		/* -(2) = -1 + -1 */
		assert_eq!( scalar_negate(&scalar("0000000000000000000000000000000000000000000000000000000000000002"))
			  , scalar_plus(&Scalar::MAX, &Scalar::MAX)
			  );

		/* -(3) = -3 */
		assert_eq!( scalar_negate(&scalar("0000000000000000000000000000000000000000000000000000000000000003"))
			  , scalar("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD036413E")
			  );
	}
}
