use hashes::sha2::sha256;

pub fn tagged_hash(tag: &str, message: &[u8]) -> [u8; 32] {
	let sha_tag = sha256::hash(tag.as_bytes()).into_bytes();
	let mut buf = Vec::new();

	buf.extend(sha_tag);
	buf.extend(sha_tag);
	buf.extend(message);

	let fin_hash = sha256::hash(&buf);
	let mut fin_buf: [u8; 32] = [0; 32];
	fin_buf.clone_from_slice(&fin_hash.into_bytes());

	fin_buf
}

#[cfg(test)]
mod test {
	use hex;
	use super::*;

	#[test]
	fn test_tagged_hash() {
		assert_eq!( tagged_hash("KeyAgg list", &hex::decode("02659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b302c6b754b20826eb925e052ee2c25285b162b51fdca732bcf67e39d647fb6830ae").expect(""))
			  , hex::decode("63dcede501945b7d89a7c6cd70c1406fed777f3c4c50d542b6ead917b6268e5e").expect("").as_slice()
			  );
		assert_eq!( tagged_hash("KeyAgg coefficient", &hex::decode("63dcede501945b7d89a7c6cd70c1406fed777f3c4c50d542b6ead917b6268e5e02659a69ea86e2f183895be58802e203eff51956e931c6282ed77ab4c4385711b3").expect(""))
			  , hex::decode("c65b8335a1e9af6d6c365f0ccb32fff99e3d8695c01b334925ad0fe30ed9adef").expect("").as_slice()
			  );
	}
}
