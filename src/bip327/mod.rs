use secp256k1::PublicKey;
use secp256k1::Scalar;
use secp256k1::Secp256k1;
use secp256k1::SecretKey;
use secp256k1::Verification;
use secp256k1::scalar::OutOfRangeError;
use super::bip340::tagged_hash;
use super::scalars::scalar_negate;
use super::scalars::scalar_plus;

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

		/* tacc' = t + g*tacc */
		let tacc_prime = scalar_plus( &t
					    , &if g { tacc.clone() } else { scalar_negate(tacc) }
					    );

		Some(
			KeyAggContext {
				q: q_prime,
				tacc: tacc_prime,
				gacc: gacc_prime
			}
		)
	}
	/* BIP-327 GetXonlyPubKey */
	pub(crate)
	fn get_xonly_pubkey(&self) -> [u8; 32] {
		let KeyAggContext{q: q, tacc: _, gacc: _} = self;
		let ser = q.serialize();
		ser[1..33].try_into().expect("constant bounds")
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
	let l = hash_keys(pk);

	let pk2 = get_second_key(pk);

	let mut a = Vec::new();

	for pk_prime in pk {
		/* KeyAggCoeffInternal */
		if pk2 < pk.len() && pk_prime == &pk[pk2] {
			a.push(Scalar::ONE);
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
		tacc: Scalar::ZERO,
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

	fn point_txt(pk_s: &str) -> PublicKey {
		let buf = hex::decode(pk_s)
		.expect("Test iput must be hex");
		PublicKey::from_slice(&buf)
		.expect("Test input must be a non-infinite point")
	}

	fn key_agg_txt(pk_s: &[&str]) -> KeyAggContext {
		let mut pk = Vec::new();
		for pk_s1 in pk_s {
			pk.push(point_txt(pk_s1));
		}

		let s_ctx = Secp256k1::new();
		key_agg(&s_ctx, &pk)
	}

	fn check_key_agg(pk_s: &[&str], q_s: &str) {
		let result = key_agg_txt(pk_s);
		let q = point_txt(q_s);

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

	fn check_apply_tweak( pk_s: &[&str] // public keys
			    , t_s: &[(&str, bool)] // tweaks and is-xonly flags
			    , q_s: &str
			    , gacc: bool
			    ) {
		let s_ctx = Secp256k1::new();

		let ini = key_agg_txt(pk_s);
		let fin = t_s.iter().fold(ini, |prev, (t_s, is_xonly)| {
			let t = hex::decode(t_s)
			.expect("tweak mnust be hex")
			.try_into()
			.expect("tweak must be 32 bytes");

			prev.apply_tweak(&s_ctx, t, *is_xonly)
			.unwrap()
		});

		let q = point_txt(q_s);

		assert_eq!(fin.q, q);
		assert_eq!(fin.gacc, gacc);
	}

	#[test]
	fn test_apply_tweak() {
		/* https://github.com/bitcoin/bips/blob/master/bip-0327/vectors/tweak_vectors.json */
		check_apply_tweak( &[ "02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9"
				    , "02DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659"
				    , "03935F972DA013F80AE011890FA89B67A27B7BE6CCB24D3274D18B2D4067F261A9"
				    ]
				 , &[ ( "E8F791FF9225A2AF0102AFFF4A9A723D9612A682A25EBE79802B263CDFCD83BB"
				      , true
				      )
				    ]
				 , "03c7a4356ba33438b49ef0141e9f00eb8146d21ca1e4fcd7f7fecefac2ba4943de"
				 , false
				 );
		check_apply_tweak( &[ "02F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9"
				    , "02DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659"
				    , "03935F972DA013F80AE011890FA89B67A27B7BE6CCB24D3274D18B2D4067F261A9"
				    ]
				 , &[ ( "E8F791FF9225A2AF0102AFFF4A9A723D9612A682A25EBE79802B263CDFCD83BB"
				      , false
				      )
				    ]
				 , "03643547cfd6c931f47fe806570e44ffc2460d77057e1506b2b7a1ab73b7f07dfe"
				 , true
				 );
		/* Minor rant: turns out BIP-327 has NO
		 * actual test vectors for the ApplyTweak
		 * algorithm *only* --- there are test
		 * vectors for signing after tweaking,
		 * but not for the resulting key.
		 * *sigh*
		 */
	}
}
