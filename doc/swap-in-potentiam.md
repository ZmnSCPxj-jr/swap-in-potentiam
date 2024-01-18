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
* Bob node ID keypair `B = b * G`.
  - `b` and `b * G` are simply the private and public keys of the Lightning
    Network node ID of Bob.

The public keys are sorted based on the lexographic ordering of the 33-byte
compressed SEC representation.
These are `P[0]` and `P[1]`, with `P[0]` being whichever of `A` or `B` is
lesser in lexicographic comparison of their 33-byte compressed SEC
representation, and `P[1]` being the other.

We create two tapscripts: one is a two-of-two between Alice and Bob, the
other is Alice only, with a relative timelock via `OP_CHECKSEQUENCEVERIFY`:

* `<P[0]> OP_CHECKSIGVERIFY <P[1]> OP_CHECKSIG`
* `<4032 blocks> OP_CHECKSEQUENCEVERIFY OP_DROP <A> OP_CHECKSIG`

The above are the leaves in the Taproot tree, with leaf version `0xC0`.

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
> 3.  It allows future revision of this protocol to use the keyspend path for
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
>     A non-hardened derivation would prevent Alice from safely revealing
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
> As it is intended that a future revision of this specification will
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

### Spending Via 2-of-2 Tapleaf Path

TODO

### Spending Via Tinelock Tapleaf Path

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
* Assemble the Merkle Tree Root as described in [BIP-341][] and
  tweak the internal public key to generate the HTLC address.

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
* `bob_provisionally_secured`
  - This output was used by Alice in a 0-conf Lightning operation
    that has not yet completed, and Bob needs to ensure it is not
    used in another swap-in-potentiam operation.
    * In this state, the transaction output is associated with a
      specific 0-conf Lightning operation.
      If that operation aborts due to a disconnection or restart of
      Bob, then the state MUST be moved to `bob_retriable`.
* `bob_secured`
  - This output was used by Alice in a 0-conf Lightning operation
    that has completed, and Bob needs to ensure it is not used in
    another swap-in-potentiam operation.
* `bob_retriable`
  - This output was used by Alice in a 0-conf Lightning operation,
    but the operation got aborted (e.g. for a 0-conf channel open,
    the connection dropped before Bob could receive the Alice-side
    signature for the funding transaction).
    Bob can now allow Alice to reuse this output.

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
  * `bob_provisionally_secured` - disallow the request.
  * `bob_secured` - disallow the request.
  * `bob_retriable` - allow the request and change state to
    `alice_moved`.
* When Alice requests a 0-conf Lightning operation on a
  swap-in-potentiam UTXO, if the state is below, Bob MUST:
  * Unknown - allow the request and set state to
    `bob_provisionally_secured`.
  * `unconfirmed_alice_change` - reject the request.
  * `confirmed_alice_change` - allow the request and set state
    to `bob_provisionally_secured`.
  * `alice_moved` - disallow the request.
  * `bob_provisionally_secured` - disallow the request.
  * `bob_secured` - disallow the request.
  * `bob_retriable` - allow the request and change state to
    `bob_provisionally_secured`.

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
> `bob_retriable` state.
>
> Finally, the onchain side of 0-conf Lightning operations
> have an anchor output, as noted in a previous section.

> **Rationale** Bob sets transaction outputs to
> `unconfirmed_alice_change` if they are the Alice-side change
> output from a 0-conf Lightning operation.
> Bob disallows spending from unconfirmed change outputs of
> 0-conf Lightning operations to arbitrary onchain transactions
> because Alice is allowed to create an arbitrary transaction from
> any swap-in-potentiam UTXOs it owns, including from UTXOs that
> are currently unconfirmed.
> If an unconfirmed change output from a 0-conf Lightning
> operation were allowed to be spent in an aribtrary transaction,
> however, Alice can pin the 0-conf Lightning operation, deferring
> its confirmation and possibly violating the security guarantees
> of swap-in-potentiam.

The following state transitions are also needed depending on
chain state:

* If the state of a swap-in-potentiam UTXO is
  `unconfirmed_alice_change` and the transaction it is an output
  of has confirmed "deeply enough" (Bob SHOULD use the
  `minimum_depth` setting it would use when it responds with a
  typical `accept_channel`), then Bob MUST set its state to
  `confirmed_alice_change`.

The following state transitions are also needed depending on if a
0-conf Lightning operation described in this specification
*aborts*:

* If the state of a swap-in-potentiam UTXO is
  `bob_provisionally_secured`, and it is used in a channel funding
  transaction that does not reach Bob (role taken by an LSP)
  receiving the `c=.sip.sign_funding_alice` API request, before
  the connection is interrupted (and thus aborting the channel
  open), then Bob MUST set its state to `bob_retriable`.
  - This includes the case where Bob / LSP restarts during channel
    opening before receiving `c=.sip.sign_funding_alice`.

The following state transitions are also needed depending on if a
0-conf Lightning operation described in this specification
*completes*:

* If the state of a swap-in-potentiam UTXO is
  `bob_provisionally_secured`, and it is used in a channel funding
  transaction that reaches Bob (role taken by an LSP) receiving a
  valid `c=.sip.sign_funding_alice` API request, then Bob MUST set
  its state to `bob_secured`.

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
call with the same UTXO (for example, to RBF a transaction, or
idempotency in case the client disconnects or crashes before it can
receive the signature from the LSP).
However, spending a swap-in-potentiam UTXO to an onchain address
also prevents it from being used to fund a 0-conf Lightning
operation in the future.

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
  ],
  "valid_until": "2024-01-18T14:42:24.000Z",
  "promise": "arbitrary-string-9999"
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
`min_feerate` is a [<LSPS0 onchain fee rate>][] in millisatoshis
per weight unit (or equivalently, satoshis per 1000 weight units)
and must be at least 253.

`valid_until` is a [<LSPS0 datetime>][] indicating the maximum
time that the returned parameters are still valid.
The LSP MUST return a `valid_until` time that is at least 60
minutes in the future.
The client SHOULD call `c=.sip.get_sip_info` again if its
cached swap-in-potentiam information has a `valid_until` that
is less than 10 minutes into the future.

`promise` is an arbitrary JSON string that identifies this set of
the returned parameters.
The LSP:

* MUST NOT use JSON string `\` escapes.
* MUST use only characters in the ASCII range 32 to 126 (except
  characters that require a JSON string `\` escape to represent).
* MUST return a JSON string of no more than 256 bytes in ASCII
  encoding.

#### Swap-in-potentiam Transaction Output Deadline

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

If a client has an existing swap-in-potentiam UTXO, and its
deadline goes below the deadline of the LSP it committed to, then
the client SHOULD use the spend-to-onchain flow to spend it
onchain to another swap-in-potentiam address to itself.
This resets the deadline, at the expense of having to re-wait for
the `min_confirmations` again.

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
* The LSP checks the other parameters of the `open_channel`, and
  if they are acceptable, sends `accept_channel`.
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
* The client calls `c=.sip.sign_funding_bob`, indicating
  the `temporary_channel_id` and the funding transaction contents.
  * The LSP validates that the spent swap-in-potentiam UTXOs can
    transition to the state `bob_provisionally_secured`, and moves
    them to that state, associating them with this channel opening
    session.
  * The LSP creates the Bob-side signatures for the funding
    transaction.
  * The LSP returns the Bob-side signatures.
* The client sends `funding_created` with the transaction ID of
  the funding transaction.
* The LSP sends `funding_signed`.
* The client generates the Alice-role signatures for the channel
  funding transaction, then calls `c=.sip.sign_funding_alice` with
  those signatures, as well as the details of the transaction.
  * The LSP validates that the funding transaction, when
    constructed, has the same transaction ID as was sent in
    `funding_created`.
  * The LSP validates that the signatures are valid for the
    funding transaction and that the transaction can be
    broadcasted.
  * The LSP moves the spent swap-in-potentiam UTXOs from the state
    `bob_provisionally_secured` to `bob_secured`.
  * The client also broadcasts the fully-signed channel funding
    transaction.
* The LSP and client exchange `channel_ready` without waiting for
  the funding transaction to confirm.
* If the deadline for the smallest-deadline transaction input
  becomes too close (as per judgement by the LSP) the LSP may
  CPFP-RBF the funding transaction via the anchor output.

#### Indicating Intent To Fund 0-Conf Backed By Swap-in-potentiam

A client first informs the LSP of an upcoming 0-conf channel
funding, by the client, with the `c=.sip.intend_to_fund_channel`
call, with parameters:

```JSON
{
  "temporary_channel_id": "123456789abcdef123456789abcdef123456789abcdef123456789abcdef",
  "promise": "arbitrary-string-9999"
}
```

`temporary_channel_id` is a JSON string containing the hex dump of
a fresh, random, 32-byte temporary channel ID that the client will
use in a subsequent [BOLT 2 `open_channel` Message][].
It MUST be exactly 64 hexadecimal characters.

The `temporary_channel_id` MUST NOT be the same as the channel ID
of any other channel, including the temporary channel ID of any
other channels currently being opened.
A client SHOULD pick the 32 bytes uniformly at random from a
high-entropy source, which would be sufficient in practice to
prevent conflicting with other channel IDs.

The LSP MUST check that the `temporary_channel_id` does not match
the channel ID of any current open channel, or any current
`temporary_channel_id` of any channel currently being opened.

`promise` is the arbitrary string returned from a previous
`c=.sip.get_sip_info` call, which identifies the set of parameters
that the client and LSP will use for this 0-conf Lightning
operation.

The LSP MUST check that the indicated `promise` string was returned
by the LSP in a previous `c=.sip.get_sip_info`, and that its
`valid_until` is still in the future.

On failure, `c=.sip.intend_to_fund_channel` may have the following
errors (error code numbers in parentheses):

* `duplicate_channel_id` (1) - the `temporary_channel_id` is
  already the (temporary or permanent) channel ID of an existing
  channel of the LSP.
* `invalid_or_unknown_promise` (2) - the `promise` does not
  identify a set of parameters returned in a previous
  `c=.sip.get_sip_info` call, or its `valid_until` is now in the
  past.
* `too_many_operations` (3) - there are too many running 0-conf
  channel funding operations that have not completed yet.

On success, `c=.sip.intend_to_fund_channel` returns the empty
object `{}`.

On success, the LSP SHOULD start a timeout of at least 10 minutes.
If the timeout is reached, and the channel opening has not reached
the step where the LSP receives and validates a corresponding
`c=.sip.sign_funding_alice` call, the LSP:

* SHOULD `error` the channel if opening has started.
* SHOULD fail any `c=.sip.sign_funding_bob` and
  `c=.sip.sign_funding_alice` calls for this channel.
* MAY reject an `open_channel` of the specified
  `temporary_channel_id` if it is a 0-conf channel open.

> **Rationale** The timeout exists to prevent a client from making
> multiple `c=.sip.intend_to_fund_channel` calls without actually
> funding any channels, thereby wasting LSP resources.

After success, the client SHOULD send an `open_channel` with the
given `temporary_channel_id`, with 0-conf and anchor commitment
types.
The client MUST ensure that it can build a funding transaction
with an onchain fee rate equal or higher than required for the
shortest-deadline UTXO it intends to spend.

After success, the LSP MUST accept an `open_channel` if all
conditions below are true:

* The timeout has not been reached yet.
* Its `temporary_channel_id` matches the one given in this call.
* The channel has the following types set:
  * `option_zeroconf`
  * `option_anchor_outputs` **OR** `option_anchors_zero_fee_htlc_tx`
* All other channel parameters (other than `temporary_channel_id`
  and `option_zeroconf`) from the client are acceptable to the
  LSP.

#### Requesting Bob-side Signatures To Fund 0-conf Channel

After the client receives the [BOLT #2 `accept_channel`
Message][] from the above sequence, it requests signatures from
the LSP as Bob using the `c=.sip.sign_funding_bob` call.

The `c=.sip.sign_funding_bob` call provides information about
the structure of the upcoming funding transaction.

The funding transaction MUST have all inputs as swap-in-potentiam
addresses with the LSP as Bob.
The funding transaction has a channel funding outpoint and an
anchor output, and an optional change output.

```JSON
{
  "temporary_channel_id": "123456789abcdef123456789abcdef123456789abcdef123456789abcdef",
  "inputs": [
    {
      "prev_out": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210:2",
      "alice_pubkey": "02fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
      "amount_sat": 19920
    },
    {
      "prev_out": "9876543219fedcba9876543219fedcba9876543219fedcba9876543219fedcba:1",
      "alice_pubkey": "039876543219fedcba9876543219fedcba9876543219fedcba9876543219fedcba",
      "amount_sat": 19920
    }
  ],
  "change": {
    "amount_sat": 99999,
    "alice_pubkey": "039876543219fedcba9876543219fedcba9876543219fedcba9876543219fedcba"
  },
  "funding": {
    "amount_sat": 100000,
    "output_script": "00204321098765fedcba4321098765fedcba4321098765fedcba4321098765fedcba"
  },
  "order": "cfa",
  "nLockTime": 655454
}
```

`temporary_channel_id` is the temporary channel ID that was used
in a previous `c=.sip.intend_to_fund_channel` call, and which the
client has used in an `open_channel` to which the LSP has responded
with an `accept_channel`.

`inputs` is a non-empty array of objects describing the inputs to
the funding transaction.
Each object has three keys:

* `prev_out` is the unspent transaction output to be spent into
  the funding transaction, [<LSPS0 outpoint>][].
* `alice_pubkey` is the Alice public key for the swap-in-potentiam
  address that locks the above unspent transaction output,
  [<LSPS0 pubkey>][].
  The LSP node ID is the Bob public key.
* `amount_sat` is the amount of this unspent transaction output.

`change` is an ***optional*** object.
If absent, it indicates that there is no change output.
If specified, the object has two keys:

* `amount_sat` is the amount to put in the change output.
* `alice_pubkey` is the Alice public key for the swap-in-potentiam
  address that locks the change output.
  The LSP node ID is the Bob public key.

`funding` is an object with two keys:

* `amount_sat` is the amount to put in the channel funding output.
* `output_script` is the `scriptPubKey` of the channel funding
  output.

`order` is a string, indicating the order of the outputs.
The client SHOULD uniformly select by random one of the valid
values for `order`:

* If `change` is not specified, `order` is a two-character string
  composed of the characters `f` and `a` in any order.
  The position of `f` indicates the position of the funding output,
  and the position of `a` indicates the position of the anchor
  output.
  * `"fa"` indicates the funding output is output index 0, and the
    anchor output is output index 1.
  * `"af"` indicates the anchor output is output index 0, and the
    funding output is output index 1.
* If `change` is specified, `order` is a three-character string
  composed of the characters `f`, `a`, and `c` in any order.
  The position of `c` indicates the position of the change output.
  * `"fac"` indicates the funding output is output index 0, the
    anchor output is output index 1, and the change output is
    output index 2.
  * `"fca"` and so on.
  * `"afc"`
  * `"acf"`
  * `"cfa"`
  * `"caf"`

`nLockTime` is an unsigned 32-bit integer, indicating the value of
the `nLockTime` field of the resulting funding transaction, and
MUST be < 500,000,000 indicating it is a block height.

The LSP, on receiving this call, performs the following validation:

* `temporary_channel_id` MUST be equal to one provided in a
  previous `c=.sip.intend_to_fund_channel` that has not yet timed
  out, and the client has already sent `open_channel` and the LSP
  has responded with an `accept_channel`.
* `inputs` MUST have at least one entry.
  * Each `prev_out` MUST be an unspent transaction output which
    has been confirmed by at least `min_confirmations`.
  * Each `prev_out` MUST be able to transition to the
    `bob_provisionally_secured` state, as described in [Bob
     Storage Requirements](#bob-storage-requirements).
  * Each `prev_out` MUST have a "deadline" that is greater than
    the smallest `max_deadline`.
    The LSP MUST determine the shortest deadline among all the
    spent transaction outputs and the corresponding `max_deadline`
    and `min_feerate`.
  * Each `prev_out` MUST have a `scriptPubKey` that corresponds to
    the swap-in-potentiam address with the given `alice_pubkey`
    and the LSP node ID as the Bob public key.
* The `amount_sat` of the `funding` object MUST equal the
  `funding_satoshis` from the `open_channel` message.

The funding transaction is constructed as follows:

* `nVersion = 2`
* `nLockTime` equals the one specified in this call.
* The inputs are ordered according to the order of `inputs`.
  * `nSequence = 0xFFFFFFFD` (i.e. opt-in RBF)
  * `prevOut` is the `prev_out` of the corresponding object.
  * `scriptSig` is empty.
* The outputs are ordered according to the `order` parameter.
  * The funding output:
    * The `value` matches the `amount_sat` of the `funding`
      object.
    * The `scriptPubKey` matches the `output_script` of the
      `funding` object.
  * The anchor output has `value` and `scriptPubKey` as specified
    in the [Anchor Output](#anchor-output) section.
  * The change output, if it exists:
    * The `value` matches the `amount_sat` of the `change`
      object.
    * The `scriptPubKey` encodes the Taproot address for the
      swap-in-potentiam address with the `alice_pubkey` of the
      `change` object, and with the LSP node ID as the Bob public
      key.

To determine the "*expected minimum fee rate*" from the `inputs`
of the funding transaction and the result of
`c=.sip.get_sip_info`:

* Iterate over the `inputs`:
  * Find the input whose "deadline"
    (`txo_confirmation_height + 4032 - current_blockheight`) is
   the lowest.
   * `txo_confirmation_height` is the height of the block that
     confirms the transaction id specified in the `prev_out`.
* Find the highest `max_deadline` that is still lower than the
  above lowest deadline.
* Get the corresponding `min_feerate`.

The funding transaction MUST have a fee rate equal to or greater
than the above expected minimum fee rate.

`c=.sip.sign_funding_bob` has the following errors defined (error
`code` in parentheses):

* `unrecognized_temporary_channel_id` (1) - The specified
  `temporary_channel_id` does not correspond to an ongoing
  0-conf funding from a swap-in-potentiam address initiated by a
  previous `c=.sip.intend_to_fund_channel`, or the LSP has not
  responded with an `accept_channel` from an `open_channel`
  specifying this temporary channel ID, or the LSP has already
  received a previous valid `c=.sip.sign_funding_bob` call for
  this temporary channel ID, or the channel open has timed out.
* `invalid_prev_out` (2) - One or more of the `prev_out`s
  specified in the `inputs` has one or more of the following
  properties:
  * It is not a confirmed unspent transaction output.
  * It has been confirmed more than 4032 blocks.
  * Its `scriptPubKey` does not encode the Taproot address for
    the swap-in-potentiam address with the given `alice_pubkey`
    and with the LSP node ID as the Bob public key.
  * Its `value` does not match the given `amount_sat`.
  * Its current `state` as specified in the section [Bob Storage
    Requirements](#bob-storage-requirements) cannot validly
    transition to `bob_provisionally_secured`.
* `insufficient_confirms` (3) - One or more of the `prev_out`s
  specified in the `inputs` has not been confirmed deeply enough.
  The `error` `data` object contains a field `block_height`
  containing the block height the LSP currently sees.
  The client can retry the channel open later.
* `deadline_too_near` (4) - One or more of the `prev_out`s
  specified in the `inputs` has been confirmed so long ago that
  the deadline is lower than the smallest `max_deadline` in the
  previously-selected set of parameters from
  `c=.sip.get_sip_info`.
* `fee_too_small` (5) - The total of the outputs is greater than
  the inputs, or the difference of the input amounts minus the
  output amounts results in a fee that causes the entire
  transaction to be below the `min_feerate` required.

Otherwise, if all the above validation fails, the LSP performs
the following atomically:

* Validates that all `prev_out`s have `state`s that can transition
  to `bob_provisionally_secured`, and transitions them to
  `bob_provisionally_secured`.
* If a change output exists on the funding transaction, sets its
  `state` to `unconfirmed_alice_change`.
* Creates signatures for all `inputs`, spending the corresponding
  `prev_out` via the 2-of-2 Tapleaf Path, and signing using the
  `SIGHASH_ALL` algorithm for SegWit.
* Store the signatures, the funding transaction, and the funding
  transaction ID into persistent storage.

The LSP then returns an object like the following as the return
value of the `c=.sip.sign_funding_bob` call, containing the
signatures it generated:

```JSON
{
  "bob_signatures": [
    "faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450",
    "aebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450f",
    "ebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450fa"
  ]
}
```

`bob_signatures` is an array of strings.
Its length equals the length of the `inputs` array in the
parameters.

Each string in `bob_signatures` is a hex dump containing the Taproot
signature from the LSP as Bob, spending the 2-of-2 Tapleaf Path,
and signing the funding transaction using the `SIGHASH_ALL`
algorithm for SegWit.
Each string is composed of 128 hex characters, each representing a
64-byte signature.

On success of the `c=.sip.sign_funding_bob` call, the client
validates the following:

* The length of `bob_signatures` MUST exactly equal the length of the
  given `inputs` array.
* Each signature in `bob_signatures` MUST validly sign the
  corresponding input of the funding transaction via the 2-of-2
  Tapleaf Path using `SIGHASH_ALL`.

If the above validation fails, the client MUST abort the channel
opening by sending a [BOLT 1 `error`  Message][] with the
`temporary_channel_id`, and SHOULD report an LSP-side error to the
user.

Otherwise, if all `bob_signatures` returned by
`c=.sip.sign_funding_bob` are valid, the client SHOULD continue
with the channel opening flow, sending a [BOLT 2 `funding_created`
Message][].

The LSP validates the [BOLT 2 `funding_created` Message][] as
follows:

* The `funding_txid` MUST equal the transaction ID of the funding
  transaction constructed above.
* The `funding_output_index` MUST equal the index of the character
  `f` in the `order` parameter of `c=.sip.sign_funding_bob`.

The LSP MAY perform the above validation by validating the
resulting permanent channel ID (which is the `funding_txid` with
the last two bytes XORed with the `funding_output_index`).

If the above validation fails, the LSP MUST send a [BOLT 1 `error`
Message][] specifying the channel being opened.
The LSP MAY send the `error` before or after sending the [BOLT 2
`funding_signed` Message][] in response to the `funding_created`
message, as long as the LSP sends the `error` *before* sending
[BOLT 2 `channel_ready` Message][].

> **Rationale** LSP implementations may be built on existing node
> software that does not provide sufficiently fine-grained hooks
> to allow the LSP implementation to immediately fail the channel
> as soon as the node implementation receives `funding_created`.
> Existing hooks might also not provide the `funding_txid` and
> `funding_output_index` of the `funding_created` message, but
> instead provide the final permanent channel ID, which is an
> encoding of both fields.
>
> The client cannot send out any HTLCs to the LSP until after the
> LSP sends `channel_ready`.
> Thus, if the client violates the above validations, the LSP is
> still safe as long as it fails the channel *before* sending
> `channel_ready`.

If the above validation succeeds, the LSP MUST send [BOLT 2
`funding_signed` Message][].

#### Providing Alice-side Signatures To Fund 0-conf Channel

Once the client receives [BOLT 2 `funding_signed` Message][] from
the LSP, the client can then complete the Alice-side signatures for
the funding transaction, resulting in a completely signed funding
transaction containing both the Alice and Bob signatures for each
input.

The client can now perform the following *in any order* or *in
parallel*:

* Send its own [BOLT 2 `channel_ready` Message][] to the LSP.
* Broadcast the completely signed funding transaction.
* Call `c=.sip.sign_funding_alice`, providing the Alice-side
  signatures.

The `c=.sip.sign_funding_alice` call takes the following
parameters:

```JSON
{
  "temporary_channel_id": "123456789abcdef123456789abcdef123456789abcdef123456789abcdef",
  "alice_signatures": [
    "ebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450fa",
    "faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450",
    "aebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450faebdc9182736450f"
  ]
}
```

`temporary_channel_id` is the temporary channel ID of this ongoing
channel open.

`alice_signatures` is an array of strings.
Its length equals the length of the `inputs` array from the previous
`c=.sip.sign_funding_bob` call, and therefore equals the number of
inputs in the funding transaction.

Each string in `alice_signatures` is a hex dump containing the Taproot
signature from the client as Alice, spending the 2-of-2 Tapleaf Path,
and signing the funding transaction using the `SIGHASH_ALL`
algorithm for SegWit.
Each string is composed of 128 hex characters, each representing a
64-byte signature.

On receiving the `c=.sip.sign_funding_alice` call, the LSP validates:

* The `temporary_channel_id` MUST be that of an ongoing 0-conf
  channel funding from a swap-in-potentiam that has had
  `c=.sip.sign_funding_bob` successfully called, and the LSP has
  sent `funding_signed`.
* The length of `alice_signatures` MUST exactly equal the number of
  inputs of the funding transaction.
* Each signature in `alice_signatures` MUST validly sign the
  corresponding input of the funding transaction via the 2-of-2
  Tapleaf Path using `SIGHASH_ALL`.

If the validation fails, the LSP fails this call and the LSP MUST
`error` the channel.
The following errors are defined for this call (error `code`s in
parentheses):

* `unrecognized_temporary_channel_id` (1) - The specified
  `temporary_channel_id` does not correspond to an ongoing
  0-conf funding from a swap-in-potentiam address where the
  client has successfully called `c=.sip.sign_funding_bob`, or the
  LSP has not responded with an `funding_signed` from an
  `funding_created` specifying this temporary channel ID, or the
  LSP has already received a previous valid
  `c=.sip.sign_funding_alice` call for this temporary channel ID,
  or the channel open has timed out.
* `invalid_alice_signatures` (2) - The length of `alice_signatures`
  is incorrect, or at least one of the `alice_signatures` is not a
  valid signature for the funding transaction.

Otherwise, the LSP performs the following atomically:

* Set all inputs of the funding transaction to `state`
  `bob_secured`.
* Store all signatures (client as Alice and LSP as Bob) into
  persistent storage, as well as the funding transaction.

Then, the LSP returns the following object from
`c=.sip.sign_funding_alice`:

```JSON
{ }
```

Once the LSP succeeds the call, the LSP MUST send a [BOLT 2
`channel_ready` Message][] for the channel, and MUST remove any
timeout it created in `c=.sip.intend_to_fund_channel`.

Once the LSP and client have exchanged `channel_ready`, the
0-conf channel funding from swap-in-potentiam process has
completed.

#### Reconnections During 0-conf Channel Funding

Until the LSP has sent `funding_signed` and the client has
received it, any disconnection and reconnection means that the
channel open has aborted, and the client has to restart the
funding.

In such a case, the LSP can simply consider the channel funding
to have aborted.

However, once the LSP has sent `funding_signed`, the BOLT
specification considers the channel to be "real" even across
disconnections, and the LSP will initiate
`channel_reestablish` for the channel.

In the case that LSP has sent `funding_signed` already before a
disconnection and reconnection occurs, and the LSP has not
received, validated, and persisted the parameters of
`c=.sip.sign_funding_alice`, the LSP MUST explicitly `error` the
channel explicitly on reconnection.

The LSP restarting would cause a disconnection as well, and
would also be an abort.

On abort, the LSP atomically performs the following:

* If `c=.sip.sign_funding_bob` was already performed:
  * Moves the `state` of the funding transaction inputs to
    `bob_retriable`.
  * Removes the change output, if any, `state` (setting it back to
    "Unknown" or not present in its persisted map).
  * Removes any persistently stored data about the funding
    transaction.

[SIP]: https://lists.linuxfoundation.org/pipermail/lightning-dev/2023-January/003810.html
[BIP-327]: https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki
[BIP-327 PubKey Agg]: https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki#user-content-Public_Key_Aggregation
[BIP-341]: https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki
[BOLT 1 `error` Message]: https://github.com/lightning/bolts/blob/master/01-messaging.md#the-error-and-warning-messages
[BOLT 2 `accept_channel` Message]: https://github.com/lightning/bolts/blob/master/02-peer-protocol.md#the-accept_channel-message
[BOLT 2 `channel_ready` Message]: https://github.com/lightning/bolts/blob/master/02-peer-protocol.md#the-channel_ready-message
[BOLT 2 `funding_created` Message]: https://github.com/lightning/bolts/blob/master/02-peer-protocol.md#the-funding_created-message
[BOLT 2 `funding_signed` Message]: https://github.com/lightning/bolts/blob/master/02-peer-protocol.md#the-funding_signed-message
[BOLT 2 `open_channel` Message]: https://github.com/lightning/bolts/blob/master/02-peer-protocol.md#the-open_channel-message
[BOLT 3 Anchor Output]: https://github.com/lightning/bolts/blob/master/03-transactions.md#to_local_anchor-and-to_remote_anchor-output-option_anchors

[<LSPS0 datetime>]: ../LSPS0/common-schemas.md#link-lsps0datetime
[<LSPS0 onchain fee rate>]: ../LSPS0/common-schemas.md#link-lsps0onchain_fee_rate
[<LSPS0 outpoint>]: ../LSPS0/common-schemas.md#link-lsps0outpoint
[<LSPS0 pubkey>]: ../LSPS0/common-schemas.md#link-lsps0pubkey
