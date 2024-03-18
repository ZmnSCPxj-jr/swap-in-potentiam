use secp256k1::PublicKey;
use secp256k1::Scalar;
use secp256k1::Secp256k1;
use secp256k1::SecretKey;
use secp256k1::Verification;
use secp256k1::scalar::OutOfRangeError;
use super::bip340::tagged_hash;

pub(crate) struct KeyAggContext {
	q: PublicKey,
	tacc: Scalar,
	gacc: bool /* true == 1, false == -1 mod n */
}

impl KeyAggContext {
	/* BIP-327 ApplyTweak.  */
	pub(crate)
	fn apply_tweak<C>( &self
			 , secp256k1: &Secp256k1<C>
			 , tweak: [u8; 32]
			 , is_xonly_t: bool
			 ) -> Option<Self>
				where C: Verification {
		let KeyAggContext{q, tacc, gacc} = self;
		let g = if is_xonly_t && !has_even_y(q) {
			false /* == -1 mod n */
		} else {
			true /* == 1 */
		};

		let t = Scalar::from_be_bytes(tweak).ok()?;

		/* g*Q */
		let q_part1 = if g {
			q.negate(secp256k1)
		} else {
			q.clone()
		};
		/* g*Q + t*G */
		let q_prime = q_part1.add_exp_tweak(secp256k1, &t)
		.ok()?;

		let gacc_prime = if !g /* => g == -1 mod n */ {
			!*gacc /* negate the sign.  */
		} else {
			*gacc /* keep the sign.  */
		};

		let tacc_prime = if tacc == &Scalar::ZERO {
			/* secp256k1 library does not support
			 * tweaking of scalar, only tweaking of
			 * private key.
			 */
			t
		} else {
			/* g*tacc */
			let tacc_prime_second = if g {
				/* No negation, just copy.  */
				tacc.clone()
			} else {
				/* Need to negate!
				 * We already checked if tacc
				 * was zero above, and the
				 * only case where the conversion
				 * from Scalar to SecretKey would
				 * fail is if the Scalar is 0.
				 */
				Scalar::from(
					SecretKey::from_slice(
						&tacc.clone().to_be_bytes()
					).expect("already checked 0")
					.negate()
				)
			};
			/* Do we have to add t?  */
			if t == Scalar::ZERO {
				tacc_prime_second
			} else {
				let sum = SecretKey::from_slice(
					&t.to_be_bytes()
				).expect("already checked 0")
				.add_tweak(&tacc_prime_second);
				/* add_tweak can fail if the sum
				 * is zero.
				 * Scalar has no addition operation
				 * (or negation, or multiplication,
				 * or....) so we convert to SecretKey,
				 * but now the problem is that
				 * SecretKey does not allow zero.
				 */
				match sum {
					/* This error only happens
					 * if the sum is zero.
					 */
					Err(_) => Scalar::ZERO,
					Ok(sum) => Scalar::from(sum)
				}
			}
		};

		Some(
			KeyAggContext {
				q: q_prime,
				tacc: tacc_prime,
				gacc: gacc_prime
			}
		)
	}
}

fn has_even_y(q: &PublicKey) -> bool {
	let ser = q.serialize();
	return ser[0] == 0x02;
}

/* BIP-327 KeyAgg */
pub(crate) fn key_agg<C>( secp256k1: &Secp256k1<C>
			, pk: &[PublicKey]
			) -> KeyAggContext
	where C: Verification
{
	let scalar_one: Scalar = Scalar::from_be_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]).expect("constant");
	let scalar_zero: Scalar = Scalar::from_be_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).expect("constant");

	let l = hash_keys(pk);

	let pk2 = get_second_key(pk);

	let mut a = Vec::new();

	for pk_prime in pk {
		/* KeyAggCoeffInternal */
		if pk2 < pk.len() && pk_prime == &pk[pk2] {
			a.push(scalar_one);
		} else {
			let mut buf = Vec::new();
			buf.extend_from_slice(&l);
			buf.extend_from_slice(&pk_prime.serialize());
			let hash = tagged_hash("KeyAgg coefficient", &buf);
			a.push(Scalar::from_be_bytes(hash)
			.expect("unlikely scalar greater than n"));
		}
	}
	assert!(a.len() == pk.len());

	let mut q = pk[0].mul_tweak(secp256k1, &a[0])
	.expect("Unlikely tweak of 0.");
	for i in 1..a.len() {
		q = q.combine(
			&pk[i].mul_tweak(secp256k1, &a[i])
			.expect("Unlikely tweak of 0.")
		).expect("Unlikely key cancellation");
	}

	return KeyAggContext{
		q: q,
		tacc: scalar_zero,
		gacc: true
	};
}

fn hash_keys(pk: &[PublicKey]) -> [u8; 32] {
	let mut buf = Vec::new();
	for pk1 in pk {
		buf.extend_from_slice(&pk1.serialize());
	}
	tagged_hash("KeyAgg list", &buf)
}

/* Returns the index to the second public key, or
 * out-of-range if no second public key.
 */
fn get_second_key(pk: &[PublicKey]) -> usize {
	for i in 1..pk.len() {
		if pk[i] != pk[0] {
			return i;
		}
	}
	return pk.len();
}

#[cfg(test)]
mod tests {
	use hex;
	use super::*;

	fn check_key_agg(pk_s: &[&str], q_s: &str) {
		let mut pk = Vec::new();
		for pk_s1 in pk_s {
			let buf = hex::decode(pk_s1)
			.expect("Test input must be hex");
			let pk1 = PublicKey::from_slice(&buf)
			.expect("Test input must be a point");
			pk.push(pk1);
		}
		let buf_q = hex::decode(q_s)
		.expect("Test input must be hex");
		let q = PublicKey::from_slice(&buf_q)
		.expect("Test input must be a point");

		let s_ctx = Secp256k1::new();
		let result = key_agg(&s_ctx, &pk);

		assert_eq!(result.q, q);
	}

	#[test]
	fn test_key_agg() {
		/* https://github.com/bitcoin/bips/blob/master/bip-0327/vectors/key_agg_vectors.json */
		check_key_agg(&["02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
				"03DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
				"023590A94E768F8E1815C2F24B4D80A8E3149316C3518CE7B7AD338368D038CA66"],
			      "0290539EEDE565F5D054F32CC0C220126889ED1E5D193BAF15AEF344FE59D4610C");
		check_key_agg(&["023590A94E768F8E1815C2F24B4D80A8E3149316C3518CE7B7AD338368D038CA66",
				"03DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
				"02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9"],
			      "036204DE8B083426DC6EAF9502D27024D53FC826BF7D2012148A0575435DF54B2B");
		check_key_agg(&["02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
				"02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
				"02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9"],
			      "02B436E3BAD62B8CD409969A224731C193D051162D8C5AE8B109306127DA3AA935");
		check_key_agg(&["02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
				"02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9",
				"03DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659",
				"03DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659"],
			      "0369BC22BFA5D106306E48A20679DE1D7389386124D07571D0D872686028C26A3E");

		/* doc/swap-in-potentiam.md */
		check_key_agg(&["02659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3",
				"02c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae"],
			      "026962aca1c57320eaa40f949928d3477f2eeb3ffdb7e3d7296c1f57608d2d2c69");
		check_key_agg(&["02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
				"02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],
			      "02f89c20245de19bd2889af0b0b4bad84bfa99e7e181ac8e9549aeebfcbb10fb1b");
		check_key_agg(&["028a3ba5c99568d26602f4cf8038371da3c86057a96eb1b6a8de1b4f1be723c236",
				"02de2848d46044aec16ea7b73233f2709f15b9bfeb720dd5d5ae595cfa51e01f15"],
			      "0359774215a479bd01274044024c52dcd5e37e50f5d3596cc374eaf5035ebc884d");
	}
}
