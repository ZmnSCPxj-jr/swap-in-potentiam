use secp256k1::PublicKey;

/* Structure for P0 and P1.  */
pub(crate)
struct P0P1 {
	p0: PublicKey,
	p1: PublicKey
}

impl P0P1 {
	pub(crate)
	fn new(alice: PublicKey, bob: PublicKey) -> Self {
		let mut a = alice.serialize();
		let mut b = bob.serialize();
		/* For simplicity, just overwrite the first
		byte to 0 so that lexicographic ordering of
		the entire array works.
		*/
		a[0] = 0;
		b[0] = 0;
		if a < b {
			P0P1{
				p0: alice,
				p1: bob
			}
		} else {
			P0P1{
				p0: bob,
				p1: alice
			}
		}
	}
}

/* Returns the tapleaf script for the 2-of-2 of the given Alice
and Bob public keys.
*/
pub(crate)
fn tapleaf_cooperative(alice: &PublicKey, bob: &PublicKey) -> Vec<u8> {
	let mut rv = Vec::new();

	let P0P1{p0, p1} = P0P1::new(*alice, *bob);

	let p0 = p0.serialize();
	let p1 = p1.serialize();
	let p0x = &p0[1..33];
	let p1x = &p1[1..33];

	rv.push(0x20); /* PUSH 32 bytes */
	rv.extend_from_slice(p0x);
	rv.push(0xAD); /* OP_CHECKSIGVERIFY */
	rv.push(0x20); /* PUSH 32 bytes*/
	rv.extend_from_slice(p1x);
	rv.push(0xAC); /* OP_CHECKSIG */

	assert_eq!(rv.len(), 68);

	rv
}

/* Returns the tapleaf script for the recovery branch of the
given Alice public key.
*/
pub(crate)
fn tapleaf_alice_recovery(alice: &PublicKey) -> Vec<u8> {
	let mut rv = Vec::new();

	let a = alice.serialize();
	let ax = &a[1..33];

	rv.push(0x03); /* PUSH 3 bytes */
	rv.extend_from_slice(&[0xC0, 0x0F, 0x00]); /* 4032, little endian */
	rv.push(0xB2); /* OP_CHECKSEQUENCEVERIFY */
	rv.push(0x75); /* OP_DROP */
	rv.push(0x20); /* PUSH 32 bytes */
	rv.extend_from_slice(ax);
	rv.push(0xAC); /* OP_CHECKSIG */

	assert_eq!(rv.len(), 40);

	rv
}

#[cfg(test)]
mod tests {
	use hex;
	use super::*;

	fn pubkey(h: &str) -> PublicKey {
		let buf = hex::decode(h)
		.expect("Test should use hex.");
		PublicKey::from_slice(&buf)
		.expect("Test should give valid point")
	}

	fn check_p0p1_sorting(a_s: &str, b_s: &str, p0_s: &str, p1_s: &str) {
		let a = pubkey(a_s);
		let b = pubkey(b_s);
		let P0P1 {p0, p1} = P0P1::new(a, b);
		assert_eq!(p0, pubkey(p0_s));
		assert_eq!(p1, pubkey(p1_s));
	}

	#[test]
	fn test_p0p1_sorting() {
		/* doc/swap-in-potentiam.md */
		check_p0p1_sorting(
			/*alice*/ "02c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae",
			/*bob*/ "03659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3",

			"03659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3",
			"02c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae"
		);
		check_p0p1_sorting(
			/*alice*/ "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
			/*bob*/ "02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",

			"02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
			"02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
		);
		check_p0p1_sorting(
			/*alice*/ "038a3ba5c99568d26602f4cf8038371da3c86057a96eb1b6a8de1b4f1be723c236",
			/*bob*/ "03de2848d46044aec16ea7b73233f2709f15b9bfeb720dd5d5ae595cfa51e01f15",

			"038a3ba5c99568d26602f4cf8038371da3c86057a96eb1b6a8de1b4f1be723c236",
			"03de2848d46044aec16ea7b73233f2709f15b9bfeb720dd5d5ae595cfa51e01f15"
		);
	}

	#[test]
	fn test_tapleaf_cooperative() {
		assert_eq!( tapleaf_cooperative(&pubkey("02c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae"),
						&pubkey("03659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3")),
			   hex::decode("20659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3AD20c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830aeAC")
			   .expect("Test gives hex"));
	}
	#[test]
	fn test_tapleaf_alice_recovery() {
		assert_eq!( tapleaf_alice_recovery(&pubkey("02c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae")),
			    hex::decode("03c00f00b27520c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830aeAC")
			    .expect("Test gives hex"));
	}
}
