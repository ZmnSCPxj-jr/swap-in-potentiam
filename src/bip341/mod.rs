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
		let left = TapTreeLeaf(TapLeaf::new(version0, script0));
		let right = TapTreeLeaf(TapLeaf::new(version1, script1));
		TapTreeBranch(Box::new(left), Box::new(right))
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
