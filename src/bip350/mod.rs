use super::Network;

const TABLE: [char; 32] =
[ 'q', 'p', 'z', 'r', 'y', '9', 'x', '8'
, 'g', 'f', '2', 't', 'v', 'd', 'w', '0'
, 's', '3', 'j', 'n', '5', '4', 'k', 'h'
, 'c', 'e', '6', 'm', 'u', 'a', '7', 'l'
];/* "qpzry9x8gf2tvdw0s3jn54khce6mua7l" */

/* BIP-173 */
const BECH32_CONSTANT: u32 = 0x1;
/* BIP-350 */
const BECH32M_CONSTANT: u32 = 0x2bc830a3;
fn network_to_hrp(n: Network) -> String {
	match n {
		Network::Mainnet => "bc".to_string(),
		Network::Testnet => "tb".to_string(),
		Network::Regtest => "bcrt".to_string()
	}
}
/* BIP-173 bech32_hrp_expand function.  */
fn bech32_hrp_expand(hrp: &str) -> Vec<u8> {
	let mut buf = Vec::new();
	/* BIP173 (which BIP350 depends on) specifies the use of
	US-ASCII encoding for human-readable parts (and implicitly
	for the entire bech32 string, since the data part is all
	encoded in US-ASCII chars).

	Rust itself assumes UTF-8 encoding for strings.

	Now, if the input is US-ASCII, UTF-8 iteration is the
	same as US-ASCII iteration.
	However, we do need some external code to ensure that
	the input HRP string *is* indeed US-ASCII.

	Fortunately, for our specific use-case, we only need a
	BECH32m *en*coder, not a decoder, and we know the HRP
	is indeed US-ASCII (see network_to_hrp func above).
	*/
	for mut c in hrp.chars() {
		c.make_ascii_lowercase();
		buf.push((c as u8) >> 5);
	}
	buf.push(0x00);
	for mut c in hrp.chars() {
		c.make_ascii_lowercase();
		buf.push((c as u8) & 0x1F);
	}

	buf
}
/* BIP-173 BIP-350 */
const GEN: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
fn bech32_polymod(values: &[u8]) -> u32 {
	let mut chk: u32 = 1;
	for v in values {
		let b = chk >> 25;
		chk = ((chk & 0x1ffffff) << 5) ^ (*v as u32);
		for i in 0..=4 {
			if ((b >> i) & 1) == 1 {
				chk = chk ^ GEN[i];
			}
		}
	}

	chk
}
/* BIP-173 BIP-350 */
fn bech32_create_checksum( constant: u32
			 , hrp: &str
			 , data_u5: &[u8]
			 ) -> [u8; 6] {
	let mut values = Vec::new();
	values.extend(bech32_hrp_expand(hrp));
	values.extend(data_u5);
	values.extend([0,0,0,0,0,0]);

	let polymod = bech32_polymod(&values) ^ constant;

	let mut rv: [u8; 6] = [0,0,0,0,0,0];
	for i in 0..=5 {
		rv[i] = ((polymod >> (5 * (5 - i))) & 0x1F) as u8;
	}
	rv
}

fn u8_to_u5(u8arr: &[u8]) -> Vec<u8> {
	let mut rv: Vec<u8> = Vec::new();

	let mut bitoff: isize = -5;
	let mut word: u16 = 0;

	for byte in u8arr {
		word = (word << 8) | (*byte as u16);
		bitoff = bitoff + 8;

		while bitoff >= 0 {
			let u5 = ((word >> bitoff) & 0x1F) as u8;
			rv.push(u5);
			bitoff = bitoff - 5;
		}
	}
	if bitoff > -5 {
		word = word << 8;
		bitoff = bitoff + 8;

		let u5 = ((word >> bitoff) & 0x1F) as u8;
		rv.push(u5);
	}

	rv
}

fn u5_to_bech32(u5arr: &[u8]) -> String {
	u5arr.iter()
		.map(|u5| TABLE[*u5 as usize])
		.collect()
}

fn encode_segwit_core( n: Network
		     , version: u8
		     , program: &[u8]
		     ) -> String {
	let hrp = network_to_hrp(n);
	let constant = if version == 0 {
		BECH32_CONSTANT
	} else {
		BECH32M_CONSTANT
	};
	let program_u5 = u8_to_u5(program);

	let mut data_part = Vec::new();
	data_part.push(version);
	data_part.extend(program_u5);

	let checksum = bech32_create_checksum(
		constant, &hrp, &data_part
	);

	hrp +
	"1" +
	&u5_to_bech32(&data_part) +
	&u5_to_bech32(&checksum)
}

/** program does *not* include the SegWit version push
nor the OP_PUSH opcode.
It is, strictly, the pushed program in the SegWit
template.

This will fail only if `version` is invalid.
'version' must be between 0 to 16, inclusive.

If 'version' is 0, then the BIP-173 "bech32" encoding
is used.
If 'version' is non-0, then the BIP-350 "bech32m"
encoding is used.
*/
pub(crate)
fn encode_segwit( n: Network
		, version: u8
		, program: &[u8]
		) -> Option<String> {
	/* BIP-173: version must be 0 to 16 inclusive.  */
	if version > 16 {
		return None;
	}
	Some(encode_segwit_core(n, version, program))
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex;

	fn test_segwit( n: Network
		      , v: u8
		      , program: &str
		      , address: &str
		      ) {
		let program = hex::decode(program)
		.expect("program should be hex");
		assert_eq!(
			encode_segwit(n, v, &program),
			Some(address.to_string())
		);
	}

	#[test]
	fn test_bip173() {
		test_segwit( Network::Mainnet
			   , 0
			   , "751e76e8199196d454941c45d1b3a323f1433bd6"
			   , "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"
			   );
		test_segwit( Network::Testnet
			   , 0
			   , "1863143c14c5166804bd19203356da136c985678cd4d27a1b8c6329604903262"
			   , "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0gdcccefvpysxf3q0sl5k7"
			   );
		test_segwit( Network::Testnet
			   , 0
			   , "000000c4a5cad46221b2a187905e5266362b99d5e91c6ce24d165dab93e86433"
			   , "tb1qqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesrxh6hy"
			   );
	}

	#[test]
	fn test_bip350() {
		test_segwit( Network::Mainnet
			   , 1
			   , "751e76e8199196d454941c45d1b3a323f1433bd6751e76e8199196d454941c45d1b3a323f1433bd6"
			   , "bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7kt5nd6y"
			   );
		test_segwit( Network::Mainnet
			   , 16
			   , "751e"
			   , "bc1sw50qgdz25j"
			   );
		test_segwit( Network::Mainnet
			   , 2
			   , "751e76e8199196d454941c45d1b3a323"
			   , "bc1zw508d6qejxtdg4y5r3zarvaryvaxxpcs"
			   );
		test_segwit( Network::Testnet
			   , 1
			   , "000000c4a5cad46221b2a187905e5266362b99d5e91c6ce24d165dab93e86433"
			   , "tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c"
			   );
		test_segwit( Network::Mainnet
			   , 1
			   , "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
			   , "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0"
			   );
	}
}
