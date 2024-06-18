/*!
The `address` module contains simple interfaces to *only*
derive a swap-in-potentiam address from the user public key
`alice` and some fixed LSP node ID `bob`.
*/
use secp256k1::PublicKey;
use secp256k1::Secp256k1;
use secp256k1::Verification;
use super::Network;
use super::bip327;
use super::bip340;
use super::bip341;
use super::bip350;
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

fn get_aggkey_and_tweak<C>( secp256k1: &Secp256k1<C>
			  , alice: &PublicKey
			  , bob: &PublicKey
			  ) -> (bip327::KeyAggContext, [u8; 32])
	where C: Verification
{
	let root_hash = get_root_hash(alice, bob);

	let pks = vec!(alice.clone(), bob.clone());
	let aggkey = bip327::key_agg(secp256k1, &pks);

	let xonly_aggkey = aggkey.get_xonly_pubkey();
	let tweak = {
		let mut concat = Vec::new();
		concat.extend_from_slice(&xonly_aggkey);
		concat.extend_from_slice(&root_hash);
		bip340::tagged_hash("TapTweak", &concat)
	};

	(aggkey, tweak)
}

/**
`derive_taproot_xonly_pubkey` generates the x-only
public key, returned as a 32-byte array, from the
given `alice` and `bob` public keys.

The operation may fail (return None) on the edge
case that the Taproot address becomes the point
at infinity.
The probability of that happening should be
negligibly low (to a cryptographer, i.e.
universe heat death is more likely to come
before you get that case).

The `scriptPubKey` can be derived from this by
prepending bytes 0x51 0x20, which is done by
`derive_taproot_scriptpubkey`.

An address can be derived for the returned
Taproot Xonly pubkey by running the return
value of this function to a bech32m library,
giving the SegWit version 1.
The `derive_taproot_address` function generates a
bech32m SegWitv1 "P2TR" address.
*/
pub
fn derive_taproot_xonly_pubkey<C>( secp256k1: &Secp256k1<C>
				 , alice: &PublicKey
				 , bob: &PublicKey
				 ) -> Option<[u8; 32]>
	where C: Verification
{
	let (aggkey, tweak) = get_aggkey_and_tweak(
		secp256k1, alice, bob
	);

	let final_pubkey = aggkey.apply_tweak(
		secp256k1,
		tweak,
		true
	)?;

	Some(final_pubkey.get_xonly_pubkey())
}

/**
`derive_taproot_scriptpubkey` generates the `scriptPubKey`
to be used for a swap-in-potentiam address whose Alice
(user) and Bob (LSP node ID) are the given public keys.

The operation may fail (return None) on the edge
case that the Taproot address becomes the point
at infinity.
The probability of that happening should be
negligibly low (to a cryptographer, i.e.
universe heat death is more likely to come
before you get that case).

You might use this for an Electrum-based SPV
interface; you will need to convert the result
of this function to the "script hash" required
by the `blockchain.scripthash.*` methods by
using SHA256 on the result.
*/
pub
fn derive_taproot_scriptpubkey<C>( secp256k1: &Secp256k1<C>
				 , alice: &PublicKey
				 , bob: &PublicKey
				 ) -> Option<Vec<u8>>
	where C: Verification
{
	let xonly_pubkey = derive_taproot_xonly_pubkey(
		secp256k1, alice, bob
	)?;

	let mut buf = Vec::new();
	buf.extend_from_slice(&[0x51, 0x20]);
	buf.extend_from_slice(&xonly_pubkey);
	return Some(buf);
}

/**
`derive_taproot_address` generates a pay-to-Taproot (P2TR)
address, returned as a `String`, from the given `alice`
and `bob` public keys.

Generally, `alice` is any public key whose private key
is derivable by the client (using any derivation scheme),
and `bob` is the Lightning Network node ID of the LSP.

The operation may fail (return None) on the edge
case that the Taproot address becomes the point
at infinity.
The probability of that happening should be
negligibly low (to a cryptographer, i.e.
universe heat death is more likely to come
before you get that case).
*/
pub
fn derive_taproot_address<C>( secp256k1: &Secp256k1<C>
			    , network: Network
			    , alice: &PublicKey
			    , bob: &PublicKey
			    ) -> Option<String>
	where C: Verification
{
	let program = derive_taproot_xonly_pubkey(
		secp256k1, alice, bob
	)?;
	Some(
		bip350::encode_segwit(
			network,
			1,
			&program
		).expect("can only fail if version is invalid, but version is hardcoded as 1")
	)
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
