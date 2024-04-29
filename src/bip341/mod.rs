use secp256k1::PublicKey;
use secp256k1::Scalar;
use secp256k1::Secp256k1;
use secp256k1::Verification;
use super::bip340::lift_x;
use super::bip340::tagged_hash;

pub(crate)
struct TapLeaf {
	version: u8,
	script: Vec<u8>
}
impl TapLeaf {
	pub(crate)
	fn new(version: u8, script: Vec<u8>) -> Self
	{ TapLeaf{version, script} }
}

pub(crate)
enum TapTree {
	TapTreeLeaf(TapLeaf),
	TapTreeBranch(Box<TapTree>, Box<TapTree>)
}
impl TapTree {
	/* swap-in-potentiam has exactly two tapleaves,
	so give a function that provides it.
	*/
	pub(crate)
	fn new_two_leaves( version0: u8, script0: Vec<u8>
			 , version1: u8, script1: Vec<u8>
			 ) -> Self {
		use TapTree::TapTreeLeaf;
		use TapTree::TapTreeBranch;
		let left = Self::new_from_script(version0, script0);
		let right = Self::new_from_script(version1, script1);
		TapTreeBranch(Box::new(left), Box::new(right))
	}
	pub(crate)
	fn new_from_script( version: u8, script: Vec<u8>) -> Self {
		use TapTree::TapTreeLeaf;
		TapTreeLeaf(TapLeaf::new(version, script))
	}
	/* swap-in-potentiam also uses BIP-327, which has its
	own tweak-the-public-key code.
	*/
	pub(crate)
	fn to_hash(self) -> [u8; 32] {
		let (_, h) = taproot_tree_helper(self);
		return h;
	}
}

/* Used internally.
In terms of BIP-341, this is the ((leaf_version, script), path)
tuple returned by taproot_tree_helper.
*/
struct Info {
	leaf: TapLeaf,
	path: Vec<u8>
}

fn load_compactsize(buf: &mut Vec<u8>, s: usize) {
	if s <= 0xFC {
		buf.push(s as u8);
	} else if s <= 0xFFFF {
		buf.push(0xFD);
		let s = s as u16;
		buf.push((s & 0xFF) as u8);
		buf.push(((s >> 8) & 0xFF) as u8)
	} else if s <= 0xFFFFFFFF {
		buf.push(0xFE);
		let s = s as u32;
		buf.push((s & 0xFF) as u8);
		buf.push(((s >> 8) & 0xFF) as u8);
		buf.push(((s >> 16) & 0xFF) as u8);
		buf.push(((s >> 24) & 0xFF) as u8);
	} else {
		buf.push(0xFF);
		let s = s as u64;
		buf.push((s & 0xFF) as u8);
		buf.push(((s >> 8) & 0xFF) as u8);
		buf.push(((s >> 16) & 0xFF) as u8);
		buf.push(((s >> 24) & 0xFF) as u8);
		buf.push(((s >> 32) & 0xFF) as u8);
		buf.push(((s >> 40) & 0xFF) as u8);
		buf.push(((s >> 48) & 0xFF) as u8);
		buf.push(((s >> 56) & 0xFF) as u8);
	}
}

fn taproot_tree_helper(script_tree: TapTree) -> (Vec<Info>, [u8; 32]) {
	use TapTree::TapTreeLeaf;
	use TapTree::TapTreeBranch;
	match script_tree {
		TapTreeLeaf(TapLeaf{version, script}) => {
			let mut buf = Vec::new();
			buf.push(version);
			/* ser_script  */
			load_compactsize(&mut buf, script.len());
			buf.extend_from_slice(&script);

			let h = tagged_hash("TapLeaf", &buf);

			( vec!(Info{leaf: TapLeaf{version, script}, path: Vec::new()})
			, h
			)
		},
		TapTreeBranch(tree_left, tree_right) => {
			let (left, left_h) = taproot_tree_helper(*tree_left);
			let (right, right_h) = taproot_tree_helper(*tree_right);

			let mut ret = Vec::new();
			for Info{leaf: l, path: mut c} in left {
				c.extend_from_slice(&right_h);
				ret.push(Info{
					leaf: l,
					path: c
				});
			}
			for Info{leaf: l, path: mut c} in right {
				c.extend_from_slice(&left_h);
				ret.push(Info{
					leaf: l,
					path: c
				});
			}

			let mut buf = Vec::new();
			if right_h < left_h {
				buf.extend_from_slice(&right_h);
				buf.extend_from_slice(&left_h);
			} else {
				buf.extend_from_slice(&left_h);
				buf.extend_from_slice(&right_h);
			}

			( ret
			, tagged_hash("TapBranch", &buf)
			)
		}
	}
}

pub(crate)
enum Bit { Bit0, Bit1 }

pub(crate)
fn taproot_tweak_pubkey<C>( s_ctx: &Secp256k1<C>
			  , pubkey: &[u8; 32]
			  , h: &[u8]
			  ) -> Option<(Bit, [u8; 32])>
				where C: Verification {
	// t = int_from_bytes(tagged_hash("TapTweak", pubkey + h))
	let mut concat = Vec::new();
	concat.extend_from_slice(pubkey);
	concat.extend_from_slice(h);
	let t = Scalar::from_be_bytes(
		tagged_hash("TapTweak", &concat)
	).ok()?;

	// P = lift_x(int_from_bytes(pubkey))
	let capital_p = lift_x(pubkey)?;

	// Q = point_add(P, point_mul(G, t))
	let capital_q = capital_p.mul_tweak(s_ctx, &t).ok()?;

	let capital_q_ser = capital_q.serialize();
	let capital_q_x = capital_q_ser[1..33].try_into().expect("constant array indices");

	let rv = ( if has_even_y(&capital_q) {Bit::Bit0} else {Bit::Bit1}
		 , capital_q_x
		 );
	Some(rv)
}
// TODO: factor out this common code in BIP-327 and BIP-341
fn has_even_y(q: &PublicKey) -> bool {
	let ser = q.serialize();
	return ser[0] == 0x02;
}

pub(crate)
fn taproot_output_script<C>( s_ctx: &Secp256k1<C>
			   , internal_pubkey: &[u8; 32]
			   , script_tree: Option<TapTree>
			   ) -> Option<Vec<u8>>
				where C: Verification {
	let h = match script_tree {
		None => { Vec::new() },
		Some(t) => {
			let (_, h) = taproot_tree_helper(t);
			h.to_vec()
		}
	};
	let (_, output_pubkey) = taproot_tweak_pubkey( s_ctx
						     , internal_pubkey
						     , &h
						     )?;
	let mut buf = Vec::new();
	buf.extend_from_slice(&[ 0x51 // SegWit v1
			       , 0x20 // Push 32 bytes
			       ]);
	buf.extend_from_slice(&output_pubkey);
	return Some(buf);
}
