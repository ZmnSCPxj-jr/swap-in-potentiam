# LSPS C= Extension: Swap-in-Potentiam

| Name        | `swap_in_potentiam`     |
| ----------- | ----------------------- |
| Version     | 1                       |
| Status      | For Implementation      |

## Motivation

[Swap-in-potentiam][SIP] is a protocol between two participants, Alice and Bob,
where Alice generates onchain addresses ostensibly owned by Alice.
Alice can then receive funds into the address from arbitrary third parties,
requiring confirmation before it can consider the funds received.
However, once the received funds are confirmed, the address may then be
immediately, without further onchain confirmations, swapped to Bob in exchange
for outgoing Lightning capacity, so that Alice can spend the amount on
Lightning.
The address is safe for address reuse.

Typically, the "Bob" role will be taken by an LSP, while the "Alice" role will be
taken by the client.
However, it is also possible for the LSP to take on the Alice role, with the
client taking on the Bob role.
Because of this possibility, the standard for generating onchain addresses
consistently refers to Alice and Bob roles, and does not refer to LSP and client.

This specification describes:

* How Alice generates an address, knowing only the public node ID of Bob.
  * How Alice can spend the funds in a UTXO in any onchain transaction,
    without cooperation from Bob.
  * How Alice and Bob construct transactions that cooperatively spend the
    funds in a UTXO.
* For client is Alice, LSP is Bob:
  * A way for Alice / client to request Bob / LSP to sign an arbitrary
    onchain transaction spending a swap-in-potentiam address.
  * A way for Alice / client to request Bob / LSP to accept an HTLC funded by
    a swap-in-potentiam address, so that Bob / LSP then forwards the HTLC back
    to Alice / client on a channel between them, so that Alice / client can put
    the funds in a Lightning channel.
  * A way for Alice / client to request Bob / LSP to accept a 0-conf channel
    funded by a swap-in-potentiam address, such that Bob / LSP can safely
    accept funds over the new 0-conf channel without any double-spend risk.
* For LSP is Alice, client is Bob:
  * A way for Alice / LSP to signal to Bob / client that it wants to open a
    0-conf channel funded by a swap-in-potentiam address, without Alice / LSP
    being required to provide an LSPS3 Promise To Unconditionally Fund 0-conf
    for that channel, as the new 0-conf channel has no double-spend risk.
* For LSP is Bob, client is third-party:
  * A way for Bob / LSP to signal an arbitrary client that it wants to
    forward a swap from a different client (acting as Alice), with the
    LSP being required to provide an LSPS3 Promise To Unconditionally Fund
    0-conf the swap between it and the third-party client.

> **Rationale** Address reuse is already expected by experienced Bitcoin users,
> thus using this protocol to generate an onchain Bitcoin address that can be
> both reused, and also whose funds can be spent over Lightning immediately, is
> beneficial.
>
> This specification consistently uses "Alice" and "Bob" roles when describing
> how to derive the onchain address and spend from it, rather than "LSP" or
> "client", because it is entirely possible for the LSP to take on the Alice
> role, with the client as Bob.
> For instance, if asynchronous receive becomes possible in the future, if the LSP knows
> that a particular offline client has a pending offline receive, and that the client
> has insufficient inbound liquidity in its channel, the LSP can prepare inbound
> liquidity to the client by putting onchain funds into a swap-in-potentiam address
> owned by the LSP-as-Alice, wait for the client to come online, then swap the fund
> with client-as-Bob to get liquidity from LSP to client, then complete the offline
> receive with the client.

### Actors

'Alice' is the logical owner of a swap-in-potentiam address.

'Bob' is a second participant in a swap-in-potentiam address, committed to by
Alice, so that Alice can immediately move funds from the swap-in-potentiam
address to a channel between Alice and Bob.

The 'client' is the API consumer.
The client can take on the Alice or Bob role.

The 'LSP' is the API provider.
The LSP can take on the Alice or Bob role.

## Constructing Addresses And Transactions

Swap-in-potentiam addresses are owned by Alice, but may be used to offer an
onchain HTLC to Bob, which Bob can then claim by offering an offchain, Lightning
HTLC.
Thus, there are two keypairs, one for each participant:

* Alice per-address keypair `A = a * G`.
  - Alice MAY derive this using any convenient derivation scheme from some root
    key.
* Bob node ID keypair `B = b * G + b' * G`.
  - `b` and `b * G` are simply the private and public keys of the Lightning
    Network node ID of Bob.
  - `b'` is a tweak provided by Alice to Bob.
    Alice may set this tweak to 0.
    - If Alice sets this to non-0, Alice MAY derive this using any convenient
      *hardened* derivation scheme from some root key.

The public keys are sorted based on the lexographic ordering of the 33-byte
compressed SEC representation.
These are `P[0]` and `P[1]`, with `P[0]` being whichever of `A` or `B` is
lesser in lexicographic comparison of their 33-byte compressed SEC
representation, and `P[1]` being the other.

We create two tapscripts: one is a two-of-two between Alice and Bob, the
other is Alice only, with a relative timelock via `OP_CHECKSEQUENCEVERIFY`:

* `<P[0]> OP_CHECKSIGVERIFY <P[1]> OP_CHECKSIG`
* `<4032 blocks> OP_CHECKSEQUENCEVERIFY OP_DROP <A> OP_CHECKSIG`

The above are the leaves in the Taproot tree, with version `0xC0`.

> **Rationale** We fix the relative timelock instead of allowing this to
> be varied in order to simplify the protocol, and for Bob to be
> better prepared for the security tradeoffs depending on the depth
> of the received funds.

### Computing The Internal Public Key

The internal public key, `Q`, is derived from `P[0]` and `P[1]`, using
the [public key aggregation scheme][BIP-327 PubKey Agg] described in
[BIP-327][].

The public key aggregation scheme is order-dependent; the order in which
the public keys are given to the public key aggregation is `P[0], P[1]`.

> **Rationale** The internal public key is an aggregate of the Alice and
> Bob public keys, which has the following advantages.
>
> 1.  It prevents use of the keyspend path, unless Alice and Bob cooperate to
>     use the keyspend path, thus either participant can force use of one
>     of the Tapscript paths.
> 2.  It prevents third parties from learning that the keyspend path is not
>     useable, as the internal public key is different for each Alice and Bob.
> 3.  It allows future version of this protocol to use the keyspend path for
>     cooperative cases, which reduces blockchain space and improves Alice and
>     Bob privacy, at the cost of greater implementation and protocol
>     complexity.
>
> One of the public keys is simply the node ID of Bob.
> This is a privacy leak, as spending via the 2-of-2 branch will reveal the
> Bob public key, and if Bob is a public node on the network, its node ID is
> known, and lets onchain observers learn that this is a swap-in-potentiam
> spend, that Bob is an LSP, and that Alice is a client of Bob.
>
> It would have been possible to design the protocol so that Bob can provide
> an arbitrary public key that is shared with a particular client, or for
> Alice to derive and provide a scalar that tweaks the Bob key.
> However:
>
> * If Bob were to provide an arbitrary public key:
>   * If Alice loses all data except for the seed of their wallet, then the
>     public key becomes unknown to Alice, and Alice would not be able to
>     recover funds.
>     Alice could request the same public key from Bob again, but now Alice
>     must trust that Bob will always provide the same public key to it.
> * If Alice were to derive a tweak somehow and provide the scalar to Bob:
>   * If Alice uses some sort of HD scheme, then Alice has to use a
>     hardened derivation from its private key.
>     A non-hardeened derivation would prevent Alice from safely revealing
>     the tweak, as the tweak would allow Bob to compute the parent private
>     key.
>     By being forced to use hardened derivation, Alice then cannot create a
>     "watch-only wallet" that knows only the root public key;
>     hardened derivation requires knowledge of the root private key.
>     Mandating the use of some tweak would also prevent Alice from using a
>     watch-only wallet.
>     Making the tweak optional would be additional complication to the
>     protocol.
>   * Similarly if the tweak were derived by Diffie-Hellman between
>     the Alice and Bob keys, Alice would need to know the private key
>     of the Alice public key to perform the derivation, again preventing
>     Alice from creating a "watch-only wallet" that only knows public
>     keys.
>
> The privacy hole here would be fixed if we can use the keyspend path
> instead of the 2-of-2 Tapscript path;
> assuming Alice does not publicize its public key, then it is not
> possible for an onchain observer to determine which Bob is being
> used, or even that this is a swap-in-potentiam address.
> As it is intended that a future version of this specification will
> enable the keyspend path, this is considered an acceptable tradeoff.

#### Test Vectors For Internal Public Key Derivation

TODO

### Computing The Address

As required by [BIP-341][], each tapscript is first serialized, then
prepended with the leaf version `0xC0`, then assembled into a Merkle Tree.
The Merkle Tree root hash is then used to tweak the above internal public
key.

We define a "tagged hash", as follows:

* `tagged_hash(tag : vector<u8>, input : vector<u8>)`
  * `= sha256(sha256(tag) || sha256(tag) || input)`
    * `||` denotes concatenation of byte vectors.
  * If `tag` is shown as a string, it is the ASCII representation of the
    string without a terminating `nul` byte.

Compute the tapleaf hashes for each of the two tapscripts above, as follows:

* `tagged_hash("TapLeaf", (0xc0 || script))`

Determine which tapleaf hash is lexicographically lesser than the other.
The lower one is `h[0]` and the higher one is `h[1]`.

The Merkle Tree root is then:

* `r = tagged_hash("TapBranch", (h[0] || h[1]))`

The internal public key, `Q`, described in the previous section,
should then be tweaked, as follows, to generate the Taproot public
key:

* `S = Q + tagged_hash("TapTweak", Q || r)`

Determine the sign of the Y coordinate of `S` (this is necessary later
on spending), then extract the X coordinate.
The X coordinate of `S` is then the 32-byte SegWit v1 address, which
is then passed to a bech32m encoder to generate the address.

#### Test Vectors For Address Generation

TODO

### Spending Via 2-of-2 Path

TODO

### Spending Via Tinelock Path

TODO

### Anchor Output

Transactions which fund Lightning channels or fund onchain swaps MUST
have an anchor output controlled by Bob.

> **Rationale** Bob needs to ensure that the Lightning channel funding
> transaction, or onchain HTLC, spending from swap-in-potentiam
> addresses, are confirmed before the timeout of the swap-in-potentiam
> transaction output.
> Thus, it must be given an anchor output for its own security.

Anchor outputs have an amount of 330 satoshis, and has a Taproot
`scriptPubKey`.
Logically, they are `Bob || CSV(16)`.

The anchor output is described in [BOLT 3 Anchor Output][].
In this context, the `remote_funding_pubkey` is the Bob key
(Bob public node ID).
Only Bob is given an anchor output.

Thus, a channel funding transaction has the following outputs:

* The channel funding outpoint.
* The Bob anchor output.
* (Optional) The Alice change output.

Similarly, an onchain HTLC transaction would have three outputs:

* The onchain HTLC.
* The Bob anchor output.
* (Optional) The Alice change output.

### Onchain HTLC

One possible option for 0-conf Lightning operations is to transfer
control of the onchain funds to Bob, by instantiating an onchain
HTLC funded from swap-in-potentiam funds, and having Bob immediately
send an in-Lightning HTLC towards Alice, or to any destination that
Alice wants to use.

The HTLC output is a Taproot output (SegWit v1 address).
The HTLC has Bob taking the hashlock branch with
SHA256 hash `h` and Alice taking the timelock branch that ends at
absolute blockheight `t`.

* Alice generates a per-swap keypair, the public key is labelled
  `A[swap]`.
* Bob similarly generates a per-swap keypair, the public key is
  labelled `B[swap]`.
* The lesser of `A[swap]` and `B[swap]`, in the lexographical
  order of their 33-byte compressed SEC encoding, is `P[swap][0]`
  and the other is `P[swap][1]`.
* Generate the `Q[swap]` internal public key from the
  [BIP-327 PubKey Agg][] of `P[swap][0]` and `P[swap][1]`, in
  that order.
* Generate two TapScripts:
  * `OP_HASH160 <RIPEMD160(h)> OP_EQUALVERIFY <B[swap]> OP_CHECKSIG`
    (hashlock branch)
  * `<t> OP_CHECKLOCKTIMEVERIFY OP_DROP <A[swap]> OP_CHECKSIG`
    (timelock branch)

TODO

## Bob Storage Requirements

Bob MUST retain the following data, for its own security:

* A mapping between a transaction output `txid:vout`, to a
  `state` described later.
  * If the transaction output has been spent onchain, Bob MAY
    forget its mapping if the *spending* transaction has already
    confirmed for 100 blocks.
    Bob MUST otherwise remember the transaction output if it
    is still unspent.

> **Rationale** The 100 blocks is the same as coinbase maturity,
> which assumes that chain reorganizations cannot be as large as
> 100 blocks.

The `state` is an enumeration of the following.
States are just human-readable labels, though Bob MAY use any other
equivalent representation:

* Unknown
  - Not an actual state; it means the output is currently not in the
    mapping.
* `unconfirmed_alice_change`
  - This output was created as a change for Alice, on a transaction
    whose other outputs are used in a 0-conf Lightning operation,
    and that operation is not yet confirmed.
* `confirmed_alice_change`
  - This output was created as a change for Alice, on a transaction
    whose other outputs are used in a 0-conf Lightning operation,
    and that operation is deeply confirmed.
* `alice_moved`
  - This output was used by Alice in an onchain operation.
* `bob_secured`
  - This output was used by Alice in a 0-conf Lightning operation,
    and Bob needs to ensure it is not used in another 0-conf
    Lightning operation.
* `bob_retried`
  - This output was used by Alice in a 0-conf Lightning operation,
    but the operation got aborted (e.g. for a 0-conf channel open,
    the connection dropped before Bob could receive the Alice-side
    signature for the funding transaction).

Alice MAY request either an onchain operation or a 0-conf Lightning
operation, when specifying a swap-in-potentiam UTXO.
Bob MUST atomically do:

* When Alice requests an onchain operation on a swap-in-potentiam
  UTXO, if the state is below, Bob MUST:
  * Unknown - allow the request and set state to `alice_moved`.
  * `unconfirmed_alice_change` - reject the request.
  * `confirmed_alice_change` - allow the request and set state to
    `alice_moved`
  * `alice_moved` - allow the request and not change state.
  * `bob_secured` - disallow the request.
  * `bob_retried` - allow the request and change state to
    `alice_moved`.
* When Alice requests a 0-conf Lightning operation on a
  swap-in-potentiam UTXO, if the state is below, Bob MUST:
  * Unknown - allow the request and set state to `bob_secured`.
  * `unconfirmed_alice_change` - reject the request.
  * `confirmed_alice_change` - allow the request and set state
    to `bob_secured`.
  * `alice_moved` - disallow the request.
  * `bob_secured` - disallow the request.
  * `bob_retried` - allow the request and change state to
    `bob_secured`.

> **Rationale** Ideally, Bob needs to ensure that a transaction
> output is signed exactly once.
>
> However, it would be nice if Alice could use RBF on onchain
> transactions which do not otherwise involve Bob.
> In that case, the rule can be relaxed so that if Alice wants
> to sign multiple versions of the same transaction, Bob
> allows it, but with the rule that transaction outputs spent
> in onchain transactions are never subsequently used to
> fund 0-conf Lightning operations.
>
> Similarly, it would be nice if an unexpected disconnection
> during a multi-step 0-conf Lightning operation, like a channel
> open, did not prevent the same transaction outputs from being
> reused in an operation that succeeds, which is why there is
> `bob_retried` state.
>
> Finally, the onchain side of 0-conf Lightning operations
> have an anchor output, as noted in a previous section.

The following state transitions are also needed depending on
chain state:

* If the state of a swap-in-potentiam UTXO is
  `unconfirmed_alice_change` and the transaction it is an output
  of has confirmed "deeply enough" (Bob SHOULD use the
  `minimum_depth` setting it would use when it responds with a
  typical `accept_channel`), then Bob MUST set its state to
  `confirmed_alice_change`.

The following state transitions are also needed depending on
Lightning operation aborts:

* If the state of a swap-in-potentiam UTXO is `bob_secured`,
  and it is used in a channel funding transaction that does
  not reach Bob (role taken by an LSP) receiving the
  `c=.sip.sign_funding_alice` API request, before the connection is
  interrupted (and thus aborting the channel open), then Bob
  MUST set its state to `bob_retried`.
  - This includes the case where Bob / LSP restarts during channel
    opening before receiving `c=.sip.sign_funding_alice`.

## Client As Alice, LSP As Bob Flows

When the client takes on the Alice role and has the LSP as the
Bob role, the client can:

* Spend a (possibly unconfirmed) swap-in-potentiam transaction
  output to an arbitrary onchain transaction, without putting
  funds into Lightning.
* Determine what the LSP requires in order to accept
  swap-in-potentiam addresses for 0-conf Lightning transactions.
* Spend one or more confirmed swap-in-potentiam transaction
  outputs to a new 0-conf channel with the LSP.
* Spend one or more confirmed swap-in-potentiam transaction
  outputs to an onchain-to-offchain swap with the LSP.

### Spending Client Swap-in-potentiam UTXOs Onchain

The client can spend directly from a onchain UTXO protected by a
swap-in-potentiam address, by asking the LSP, as Bob, to sign an
arbitrary transaction spending it.

The client requests the LSP to perform such operations by calling
`c=.sip.sign_sip_onchain`, which has the parameters:

```JSON
{
  "todo": "todo"
}
```

TODO

On successful return, the LSP has, as Bob, updated its
persistently-stored `state` of the spent swap-in-potentiam UTXO to
`alice_moved`.
This state allows the client to repeat the `c=.sip.sign_sip_onchain`
call with the same UTXO (for example, to RBF a transaction).
However, spending a swap-in-potentiam UTXO to an onchain address
also prevents it from being used to fund a 0-conf Lightning
operation.

TODO

### Determining LSP Parameters For 0-Conf Lightning

For its own security, the LSP will set certain parameters.
The client uses `c=.sip.get_sip_info` to query those parameters.
`c=.sip.get_sip_info` takes no parameters `{}` and has no errors
defined.

`c=.sip.get_sip_info` results in an object like the below:

```JSON
{
  "min_confirmations": 3,
  "onchain_fee_schedule": [
    {
      "max_deadline": 288,
      "min_feerate": 50000
    },
    {
      "max_deadline": 576,
      "min_feerate": 25000
    },
    {
      "max_deadline": 1008,
      "min_feerate": 10000
    }
  ]
}
```

`min_confirmations` is a required JSON positive non-zero
integral number indicating the number of blocks that a
swap-in-potentiam transaction output must be confirmed,
before this LSP will accept it for 0-conf Lightning
operations.

The LSP:

* SHOULD NOT change the `min_confirmation` once it has
  indicated it for a particular client.
  * If it absolutely needs to change this setting,
    SHOULD only lower it and not increase it for that
    particular client.
* SHOULD set `min_confirmations` to the same value as
  it would set for `minimum_depth` in an `accept_channel`
  for a non-swap-in-potentiam-funded channel with this
  client.

> **Rationale** Sending to a swap-in-potentiam address
> is really opening a non-Lightning channel between
> Alice and Bob, with the funds initially in the Alice
> side.
> Thus, the `minimum_depth` for a Lightning channel
> should also be used for the `min_confirmations` of a
> swap-in-potentiam with Alice, as they are congruous
> operations.

`onchain_fee_schedule` is a required array of objects.
If `onchain_fee_schedule` is empty, the LSP currently
does not allow 0-conf Lightning operations with the
client.
Otherwise if the array is non-empty, the LSP allows
0-conf Lightning operations.

Each object in `onchain_fee_schedule` has two fields,
`max_deadline` and `min_feerate`, both required JSON
non-zero positive integral numbers.
Objects in the array MUST be sorted on the
`max_deadline` field in ascending order from lowest to
highest.
Objects in the array MUST NOT duplicate `max_deadline`.
`max_deadline` MUST be non-zero and less than 4032.
`min_feerate` is an onchain feerate in millisatoshis
per weight unit (or equivalently, satoshis per 1000
weight units) and must be at least 253.

The "deadline" is the number of blocks remaning before
the `OP_CHECKSEQUENCEVERIFY` branch of the
swap-in-potentiam address becomes valid.
It is `txo_confirmation_height + 4032 - current_blockheight`.

For example, suppose a transaction that sends to a
swap-in-potentiam address confirms at block height
100,000.
Now suppose the current block height is 100,200.
The deadline for that transaction output would be
100,000 + 4032 - 100,200 = 3832.

When the client wishes to spend multiple
swap-in-potentiam transaction outputs it controls in
one 0-conf Lightning operation, the deadline is the
lowest deadline among the transaction outputs.

The lowest `max_deadline` in the `onchain_fee_schedule`
array is the lowest deadline that the LSP will allow.
If the deadline for a swap-in-potentiam transaction
output is lower than the smallest `max_deadline`, the
LSP will not allow that transaction output to be used
in 0-conf Lightning operations.

The onchain feerate used depends on the schedule.
For example, suppose the `onchain_feerate_schedule`
is:

```JSON
[
  {
    "max_deadline": 288,
    "min_feerate": 50000
  },
  {
    "max_deadline": 576,
    "min_feerate": 25000
  },
  {
    "max_deadline": 1008,
    "min_feerate": 10000
  }
]
```

This means:

* If the deadline is 287 blocks or less, the LSP will
  not allow that swap-in-potentiam transaction output
  in 0-conf Lightning operations.
* If the deadline is 288 to 575 blocks (inclusive), the
  LSP demands an onchain feerate of 50,000 sat/kWU.
* If the deadline is 576 to 1007 blocks, the LSP demands
  an onchain feerate of 25,000 sat/kWU.
* If the deadline is 1008 to 4032 blocks, the LSP demands
  an onchain feerate of 10,000 sat/kWU.

The specified onchain feerate MUST be used as the minimum
onchain feerate for funding channel opens and onchain
HTLCs.
For onchain-to-offchain swaps, the onchain feerate also
determines how much the LSP will deduct from the onchain
amount, prior to sending an in-Lightning HTLC to the
client.

### Opening 0-Conf Lightning Channels Backed By Swap-in-potentiam

A client, taking on the role of Alice, may request the LSP,
taking on the role of Bob, to accept a 0-conf channel funded
unilaterally by the client.

Normally, a node accepting a 0-conf channel funded by another
node must trust that the opening node will not double-spend
the channel funding transaction before confirmation.

However, the swap-in-potentiam mechanism allows the Bob role to
ensure that a swap-in-potentiam UTXO does not get double-spent,
by Bob simply refusing to sign for a UTXO multiple times.
Thus, Bob can prevent the 0-conf channel from being double-spent
before the channel funding transaction confirms.

An LSP might have a policy of normally rejecting 0-conf channel
opens, as a precaution against this.
However, if the client indicates that it will use *only*
swap-in-potentiam UTXOs as transaction inputs to a 0-conf channel
open, with an LSP that has the Bob role in the swap-in-potentiam
address, then the LSP can safely accept that 0-conf channel open,
provided it validates that the transaction does use the specified
UTXOs.

The opening flow goes this way:

* On BOLT#8 tunnel establishment, both LSP and client negotiate
  `option_channel_type` during `init`.
* The client calls `c=.sip.intend_to_fund_channel`, indicating the
  `temporary_channel_id` of the subsequent `open_channel`.
* The client sends `open_channel` with the indicated
  `temporary_channel_id`, with `type` including `option_zeroconf`.
* The LSP checks the other parameters of the `open_channel`,
  and if they are acceptable, sends `accept_channel`.
* The client constructs the 0-conf funding transaction.
  * The transaction spends one or more confirmed (to depth
    `min_confirmations`) swap-in-potentiam UTXOs, all of which
    have the client as Alice and the LSP as Bob.
  * The transaction spends to:
    * The channel funding address, as computed per BOLT spec.
    * An anchor output controlled by the LSP.
    * Optionally, a swap-in-potentiam change address with the
      client as Alice and the LSP as Bob.
  * The transaction feerate is at least the `min_feerate` of the
    lowest-deadline UTXO spent, according to the
    `onchain_fee_schedule`.
* The client calls `c=.sip.provide_funding_tx`, indicating
  the `temporary_channel_id` and the funding transaction contents.
  * The LSP validates that the spent swap-in-potentiam UTXOs can
    transition to the state `bob_secured`, and moves them to that
    state, associating them with the channel opening session.
* The client sends `funding_created` with the transaction ID of
  the funding transaction.
  * The LSP validates that the transaction ID matches the one from
    the previous `c=.sip.provide_funding_tx`.
* The LSP sends `funding_signed`.
* The client generates the Alice-role signatures for the channel
  funding transaction, then calls `c=.sip.sign_funding_alice` with
  those signatures.
  * The LSP generates the Bob-role signatures for the channel
    funding transaction and broadcasts it.
  * The LSP removes the association of the UTXOs involved with the
    channel open, so that abort no longer moves them to
    `bob_retried` state.
  * The LSP returns the Bob-role signatures to the client.
  * The client also broadcasts the fully-signed channel funding
    transaction.
* The LSP and client exchange `channel_ready` without waiting for
  the funding transaction to confirm.
* If the deadline for the smallest-deadline transaction input
  becomes too close (as per judgement by the LSP) the LSP may
  CPFP-RBF the funding transaction via the anchor output.

[SIP]: https://lists.linuxfoundation.org/pipermail/lightning-dev/2023-January/003810.html
[BIP-327]: https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki
[BIP-327 PubKey Agg]: https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki#user-content-Public_Key_Aggregation
[BIP-341]: https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki
[BOLT 3 Anchor Output]: https://github.com/lightning/bolts/blob/master/03-transactions.md#to_local_anchor-and-to_remote_anchor-output-option_anchors
