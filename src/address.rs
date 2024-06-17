use secp256k1::PublicKey;
use secp256k1::Secp256k1;
use secp256k1::Verification;
use super::bip327;
use super::bip341;
use super::scripts;

fn get_root_hash( alice: &PublicKey
		, bob: &PublicKey
		) -> [u8; 32]

{
	let coop_tapleaf_path = scripts::tapleaf_cooperative(alice, bob);
	let recov_tapleaf_path = scripts::tapleaf_alice_recovery(alice);

	let taptree = bip341::TapTree::new_two_leaves(
		bip341::TAPROOT_TAPLEAF_VERSION, coop_tapleaf_path,
		bip341::TAPROOT_TAPLEAF_VERSION, recov_tapleaf_path
	);
	taptree.to_hash()
}

pub
fn derive_taproot_xonly_pubkey<C>( secp256k1: &Secp256k1<C>
				 , alice: &PublicKey
				 , bob: &PublicKey
				 ) -> Option<[u8; 32]>
	where C: Verification
{
	let root_hash = get_root_hash(alice, bob);

	let pks = vec!(alice.clone(), bob.clone());
	let aggkey = bip327::key_agg(secp256k1, &pks);

	let final_pubkey = aggkey.apply_tweak(
		secp256k1,
		root_hash,
		true
	)?;

	Some(final_pubkey.get_xonly_pubkey())
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

	#[test]
	fn test_testvector_roothash() {
		/* swap-in-potentiam.md
		 * Address Generation Test Vector 1
		 */
		assert_eq!( get_root_hash( /* A = */ &point_txt("02c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae")
					 , /* B = */ &point_txt("03659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3")
					 ).to_vec()
			  , hex::decode(
				/* r = */
				"9a7de09467b643aa9a636cb77488e60d822845ff38db30f8f486903fd552783b"
			    ).expect("you can see it is hex right there come on")
			  );
	}
}
