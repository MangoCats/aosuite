/* MIT License
 *
 * Copyright (c) 2021 Assign Onward
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
#ifndef CODEDEF_H
#define CODEDEF_H

#define RDT_NULL            0x00 // z - A null block value type, used as an error flag 
#define RDT_MPZ             0x01 // n - libGMP signed integer - json as a quoted ASCII decimal string, bson as an object carrying the native GMP values
#define RDT_MPQ             0x02 // r - libGMP rational fraction - json as a quoted ASCII decimal string/ASCII decimal string, bson as an object carrying the native GMP
#define RDT_STRING          0x05 // s - UTF8 encoded string - json as UTF8 string, bson as int32_t length (includes terminator) followed by UTF8 string plus null terminator
#define RDT_INT64           0x06 // i - 64 bit signed integer - json in an unquoted ASCII decimal string, bao as 8 bytes using void qToLittleEndian(qint64 src, uchar *dest)
#define RDT_RICEYINT        0x09 // v - 63 bit unsigned integer - json in an unquoted ASCII decimal string, bao as ricey code
#define RDT_BYTEARRAY       0x0B // b - ByteArray (aka octet stream) - json as hex string, bson as int32_t length followed by length octets
#define RDT_RCODE           0x0C // c - Ricey code - json as the table defined name, bson as a list of bytes: last byte has 0 in most significant bit.
#define RDT_OBJECT          0x0D // o - in json: a comma separated list of zero or more key-value pairs json encases each pair in {}, all keys unique within the object, in bson there are no separators, a key of Obterm ends the current object
#define RDT_MPZ_ARRAY       0x11 // N - array of zero or more libGMP signed integers stored as described above
#define RDT_MPQ_ARRAY       0x12 // R - array of zero or more libGMP rational fractions stored as described above
#define RDT_STRING_ARRAY    0x15 // S - array of zero or more UTF-8 encoded strings - json as quoted, escaped UTF-8 string, bson as int32_t length followed by UTF-8 string, no null terminator
#define RDT_INT64_ARRAY     0x16 // I - array of zero or more 64 bit signed integers stored as above, comma separated in json, rice code size followed by the array in bson
#define RDT_RICEYINT_ARRAY  0x19 // V - array of zero or more 63 bit unsigned integers stored as above, comma separated in json, rice code size followed by the array in bson
#define RDT_BYTEARRAY_ARRAY 0x1B // B - ByteArray (aka octet stream) - json as hex string, bson as int32_t length followed by length octets
#define RDT_RCODE_ARRAY     0x1C // C - array of zero or more Ricey codes, quoted hexadecimal and comma separated in json, starts with a rice code size followed by the array in bson
#define RDT_OBJECT_ARRAY    0x1D // O - array of zero or more objects encased in []
#define RDT_ARRAY           0x10 // * - this bit is always set in array types
#define RDT_TYPEMASK        0x0F // * - used to get the contained value type out of an array type 
#define RDT_OBTYPEMASK      0x1F // * - used to get the contained value type out of an object type

#define RCD_null_z                                     0 // dataGroup This key indicates the value is empty, has no value - usually indicates an error
#define RCD_mpz_n                                      1 // dataGroup Generic MPZ large integer, use depends on context.
#define RCD_mpq_r                                      2 // dataGroup Generic MPQ large integer fraction, use depends on context.
#define RCD_text_s                                     5 // dataGroup Generic string, use depends on context, for instance in an image object this would be the alt text
#define RCD_int64_i                                    6 // dataGroup Generic signed int64, use depends on context.
#define RCD_riceyInt_v                                 9 // dataGroup Generic unsigned 63 bit int, use depends on context.
#define RCD_data_b                                    11 // dataGroup Generic data block, use determined by the object which contains it.
#define RCD_type_c                                    12 // dataGroup Identifies an algorithm type for hash or crypto key, or other types depending on context
#define RCD_ob_o                                      13 // dataGroup Generic object, use depends on context
#define RCD_mpz_N                                     17 // dataGroup A generic array of MPZ elements
#define RCD_mpq_R                                     18 // dataGroup A generic array of MPQ elements
#define RCD_text_S                                    21 // dataGroup A generic array of string elements
#define RCD_int64_I                                   22 // dataGroup A generic array of int64 elements
#define RCD_riceyInt_V                                25 // dataGroup A generic array of riceyInt elements
#define RCD_data_B                                    27 // dataGroup A generic array of byte array element
#define RCD_type_C                                    28 // dataGroup A generic array of Ricey elements
#define RCD_ob_O                                      29 // dataGroup A generic array of object elements
#define RCD_AOShares_n                                33 // dataGroup A quantity of shares - meaning depends on context
#define RCD_name_s                                    37 // dataGroup Name as a string, use depends on context, often a key name
#define RCD_time_i                                    38 // dataGroup UTC time of block creation (microseconds since epoch x 189). May never be less than or equal to the timei of any parent block. Serves as a unique id for AOBO blocks.
#define RCD_time_I                                    54 // dataGroup UTC time of block creation (microseconds since epoch x 189). May never be less than or equal to the timei of any parent block. Serves as a unique id for AOBO blocks.
#define RCD_desc_s                                    69 // dataGroup Description of the current object, specific use depends on context
#define RCD_code_s                                   101 // dataGroup Code as a string, use depends on context, often a key type
#define RCD_data2_b                                   43 // dataGroup Additional generic data block, use determined by the object which contains it.
#define RCD_data3_b                                  107 // dataGroup Additional generic data block, use determined by the object which contains it.
#define RCD_data4_b                                   75 // dataGroup Additional generic data block, use determined by the object which contains it.
#define RCD_hashedOb_o                                45 // chainGroup An object which has its hash computed. When found in a chainBlock_o always contains a parentHash_O a time_i of when the block was signed, the hash_o is found in the object which contains the hashedOb_o.
#define RCD_hashInfo_o                                77 // chainGroup Found in hash_o, contains at least the time_i plus any other id info that will be hashed along with the hashedOb_o
#define RCD_hash_o                                   109 // chainGroup Found in chainBlock_O and similar objects which contain a signedBlock_O, contains an algorithm id (type_y) and the hash by that algorithm of the binary representation of the objects in the signedBlock_O (data_b) and a timestamp (time_i) matching the time_i in the signedBlock_O and which must not be as old or older than any parent timestamp and also should not be in the future, these time_i are unique identifiers for chainBlock_O objects.
#define RCD_parentHash_O                             125 // chainGroup Contains copies of the hash_o of one or more (more in the case of merging forked chains) parent blocks.
#define RCD_chainBlock_O                            9629 // chainGroup An Assign Onward Chain Block. With a hash_o which includes a timei, and a hashedOb_o which includes exact copied of the hash_o(s) of the parent(s)
#define RCD_chainBlock_o                            9613 // chainGroup An Assign Onward Chain Block. With a hash_o which includes a timei, and a hashedOb_o which includes exact copied of the hash_o(s) of the parent(s)
#define RCD_separableItems_O                         861 // chainGroup List of items which are only indirectly included in the hash chain
#define RCD_separableItemsHashes_O                   829 // chainGroup Hashes of separable items which do get included in the hash chain
#define RCD_SHA256_c                                 140 // algorithmIdGroup algorithm ID for SHA2 256
#define RCD_SHA3b512_c                               236 // algorithmIdGroup algorithm ID for SHA3 512
#define RCD_Blake2s128_c                             300 // algorithmIdGroup algorithm ID for Blake2s 128
#define RCD_Blake2b256_c                             364 // algorithmIdGroup algorithm ID for Blake2b 256
#define RCD_Blake2b512_c                             396 // algorithmIdGroup algorithm ID for Blake2b 512
#define RCD_ECB256_c                                 172 // algorithmIdGroup algorithm ID for Elliptic Curve ECDSA curve BrainpoolP256r1
#define RCD_RSA3072_c                                204 // algorithmIdGroup algorithm ID for 3072 bit RSA - less commonly used algorithms get longer ricey codes
#define RCD_jpg_c                                    268 // algorithmIdGroup Found in image objects, tells what format the data_b is to be interpreted as
#define RCD_png_c                                    332 // algorithmIdGroup Found in image objects, tells what format the data_b is to be interpreted as
#define RCD_success_c                               8204 // dataGroup success code
#define RCD_failure_c                              14348 // dataGroup failure code
#define RCD_ProtocolDef_O                           4221 // protocolDefGroup An Assign Onward Protocol List - should contain one or more ProtocolDef_o describing the protocols an actor is equipped to handle
#define RCD_ProtocolDef_o                           4205 // protocolDefGroup An Assign Onward Protocol Definition - defines containers and their contents including operation requirements (hashes, signatures, order of timestamps, etc.)
#define RCD_ProtocolSig_o                           4173 // protocolDefGroup A signature on an Assign Onward Protocol Definition, indicating the protocol is "approved" by the signer
#define RCD_ItemsDef_O                              4157 // protocolDefGroup An Assign Onward Protocol Item Definition - overall, defines containers and their contents including operation requirements (hashes, signatures, order of timestamps, etc.)
#define RCD_ItemsDef_o                              4141 // protocolDefGroup An Assign Onward Protocol Item Definition - overall, defines containers and their contents including operation requirements (hashes, signatures, order of timestamps, etc.)
#define RCD_ItemStructure_o                         4237 // protocolDefGroup Defines an item structure, but not contents
#define RCD_ItemContents_O                          4253 // protocolDefGroup Defines contents of an item structure, may point to other item structures for containers.
#define RCD_ActorsDef_o                             4109 // protocolDefGroup A Actor definitions
#define RCD_sendableItems_C                         4316 // protocolDefGroup A list items this actor publishes
#define RCD_receivableItems_C                       4348 // protocolDefGroup A list items this actor consumes
#define RCD_actorWriterClientId_b                   7435 // protocolActorGroup Actor id: unique id of the actor who wants to create a record (in a blockchain)
#define RCD_actorWriterServerId_b                   7467 // protocolActorGroup Actor id: unique id of the actor who handles requests to create records (in a blockchain)_b 0xBA4B // protocolActorGroup Actor id: unique id of the actor who wants to retrieve a record (from a blockchain)
#define RCD_actorReaderServerId_b                   7531 // protocolActorGroup Actor id: unique id of the actor who handles requests to retrieve records (from a blockchain)
#define RCD_blockchainId_b                          7691 // protocolActorGroup Unique id of a blockchain, often the bao of its hash_o
#define RCD_actorWriterClient_o                     7437 // protocolActorGroup Actor definition: the actor who wants to create a record (in a blockchain)
#define RCD_actorWriterServer_o                     7469 // protocolActorGroup Actor definition: the actor who wants to retrieve a record (from a blockchain)
#define RCD_actorReaderClient_o                     7501 // protocolActorGroup Actor definition: the actor who handles requests to create records (in a blockchain)
#define RCD_actorReaderServer_o                     7533 // protocolActorGroup Actor definition: the actor who handles requests to retrieve records (from a blockchain)
#define RCD_writeRequest_o                          6157 // protocolDefGroup A request to write data to storage
#define RCD_writeResponse_o                         6189 // protocolDefGroup A response to a request to write data to storage
#define RCD_readRequest_o                           6221 // protocolDefGroup A request to read data from storage
#define RCD_readResponse_o                          6253 // protocolDefGroup A response to a request to read data from storage
#define RCD_SubItemDefs_O                           4189 // protocolDefGroup inside _o or _O items, SubItemDefs_O defines the requirements for sub-items which may appear
#define RCD_recordText_s                           10245 // protocolDefGroup DⒶ0 and later: text that goes in a write request, or comes from a read request
#define RCD_recordId_i                             10246 // protocolDefGroup DⒶ0 and later: timestamp of a record when written, or used to request reading by timestamp
#define RCD_userId_b                               10379 // protocolDefGroup DⒶ1 and later: user identity - hopefully unique
#define RCD_serverId_b                             10635 // protocolDefGroup DⒶ3 and later: server identity - hopefully unique
#define RCD_OpDataLink_C                           12316 // operationGroup Links ItemContents to more uniquely defined data types used to interface with applications
#define RCD_OperReqList_O                          12317 // protocolDefGroup USE inside SubItemDefs_O Operational requirements / constraints on values found in Required or Optional items
#define RCD_OperReq_o                              12429 // protocolDefGroup USE inside SubItemDefs_O A single operational requirement / constraint on value found in an item
#define RCD_OpMemberOf_C                           12572 // operationGroup List of possible values for an RCODE
#define RCD_OpOneOrMoreOf_C                        12700 // operationGroup One or more objects of the listed types are required to appear
#define RCD_OpGreaterThan_C                        12828 // operationGroup Path defining a value, or array of values, to compare - the targeted value must be greater than all
#define RCD_OpLessThan_C                           12956 // operationGroup Path defining a value, or array of values, to compare - the targeted value must be less than all
#define RCD_OpTimeValue_c                          13068 // operationGroup The specified object should be assigned a time value, RCD_time_i indicates the time should be the current time - other values may be used in the future to indicate other types of time values.
#define RCD_OpHash_O                               13213 // operationGroup Three parameters follow, describing inputs to the hash function
#define RCD_itemRelAddr_C                        2095836 // structNavGroup Navigation from the current itemDef to an operand in an operation, sequence of item types when navigating deeper, relative step back or to root to get higher in the hiearchy
#define RCD_navUpOne_c                           2095852 // structNavGroup Navigate 'up one' level in a nested block, used in operand relative addressing.
#define RCD_navChainBlock_c                      2095756 // structNavGroup Navigate to the 'root' of this chain block, used in operand absolute addressing.
#define RCD_navJumpToBlock_c                     2095788 // structNavGroup The next item in the itemRelAddr_Y array contains a chainBlock index, navigation will continue from that block.
#define RCD_navIfPresent_c                       2095820 // structNavGroup If the next item is present, then proceed - otherwise this condition is satisfied by the absence of the optional item.
#define RCD_requestAvailServices_o                692941 // serviceDescriptors Object contains the protocol name and/or uid (some protocols will require uid, early ones will go by name).
#define RCD_availServicesList_O                   692957 // serviceDescriptors List of services provided, objects contain a type_y: service descriptor and possibly additional info like lists of items, database names, etc.
#define RCD_requestRecordStorage_o                692909 // serviceDescriptors Object describes the id of the requestor, id of the database to record in, and the data to record
#define RCD_recordStorageResult_o                 692877 // serviceDescriptors Object describes success or failure of a record storage attempt
#define RCD_requestRecordRetrieval_o              693037 // serviceDescriptors Object describes the id of the requestor, id of the database to record in, and id of the record to retrieve
#define RCD_recordRetrievalResult_o               693005 // serviceDescriptors Object describes success (with data) or failure of a record retrieval attempt
#define RCD_RangeBounds_O                          14365 // dataGroup Boundaries for valid values
#define RCD_min_i                                  14406 // dataGroup Used in range boundaries
#define RCD_min_n                                  14401 // dataGroup Used in range boundaries
#define RCD_min_r                                  14402 // dataGroup Used in range boundaries
#define RCD_max_i                                  14438 // dataGroup Used in range boundaries
#define RCD_max_n                                  14433 // dataGroup Used in range boundaries
#define RCD_max_r                                  14434 // dataGroup Used in range boundaries
#define RCD_enum_C                                 14428 // dataGroup Used to define available choices
#define RCD_enum_S                                 14421 // dataGroup Used to define available choices
#define RCD_GenesisBlock_o                        530445 // dataGroup An Assign Onward Genesis Block. With a signature_o which includes a timei, and other definitions for the chain.
#define RCD_ChainDesc_o                           534157 // dataGroup Chain Description, generally only found in the Genesis Block - descriptive items like name, symbol, number of coins represented by all shares, etc.
#define RCD_ChainFunc_o                           534413 // dataGroup Chain Functional items description, generally only found in the Genesis Block - functional items like number of shares, expiration rates, recording fees, etc.
#define RCD_EndChain_o                            538125 // dataGroup Used when reporting the last blocks in a chain, this block is a signal that all known blocks are received. Has a pzzzO and zzzO and timei of when it was reported, but contains no transaction information and will (usually) be discarded when appending new transaction blocks to its parent.
#define RCD_Symbol_s                              534149 // dataGroup A short string like ITC representing the chain.
#define RCD_CdName_s                              534181 // dataGroup A longer string like Itcoin representing the chain.
#define RCD_Tagline_s                             534213 // dataGroup A brief sentence like "The greatest blockchain ever" describing the chain
#define RCD_Description_s                         534245 // dataGroup A paragraph (say 1000-ish characters) describing the chain.
#define RCD_Icon_o                                534189 // dataGroup A square-ish image that looks good small, representing the chain.
#define RCD_Banner_o                              534221 // dataGroup A wide image that might be displayed near the top of a page, representing the chain.
#define RCD_Image_o                               534253 // dataGroup A more detailed image representing the chain.
#define RCD_CfShares_n                            534401 // dataGroup The number of shares recorded in the Genesis block.
#define RCD_CfCoins_n                             534433 // dataGroup The number of "coins" all the shares in this chain represent.
#define RCD_CfRecFee_r                            534402 // dataGroup The number of coins per byte charged to record data on this chain (usually 1/a very large number).
#define RCD_ExchAdvert_o                            8205 // contractGroup An exchange advertisement for cross chain exchange swaps (expressed in coins for coins)
#define RCD_Proposal_o                              8237 // contractGroup A share swap proposal
#define RCD_Offer_o                                 8269 // contractGroup A share swap offer
#define RCD_Acceptance_o                            8397 // contractGroup A share swap acceptance of offer
#define RCD_Cancellation_o                          9325 // contractGroup A share swap notice of cancellation (for cause, usually time expiration)
#define RCD_Identity_o                              8333 // contractGroup A cross chain identity, may be tied to share ownership but carries its own key pair
#define RCD_NineCode_z                 72057594037927936 // dataGroup A test code to check a 9 byte ricey code
#define RCD_MaxiCode_z               9223372036854775776 // dataGroup A test code to check a 9 byte ricey code
#define RCD_riceyTypes_O                         1336221 // dictionaryGroup An array object which contains the description, name and ricey code 4 LSB for each defined ricey code type in the system.
#define RCD_riceyCodes_O                         1336253 // dictionaryGroup An array object which contains the description, name and ricey code for each defined ricey code in the system.
#define RCD_type_b                               1336203 // dictionaryGroup In the dictionary definition context, _y is not defined yet, use this instead of type_y
#define RCD_group_s                              1336197 // dictionaryGroup A logical collecting of code names, can become a folder structure if necessary.


#endif // CODEDEF_H