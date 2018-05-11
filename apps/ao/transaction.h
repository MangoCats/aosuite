/* MIT License
 *
 * Copyright (c) 2018 Assign Onward
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
#ifndef TRANSACTION_H
#define TRANSACTION_H

#include <QObject>
#include "aotime.h"
#include "blockref.h"
#include "datavarlenlong.h"
#include "shares.h"
#include "pageref.h"
#include "participant.h"
#include "pubkey.h"
#include "random.h"


/**
 * @brief The ShareSource class - identifies a source of shares
 */
class ShareSource : public DataVarLenLong
{
    Q_OBJECT
public:
    explicit  ShareSource( QObject *p = nullptr) : DataVarLenLong( AO_SHARE_SOURCE, QByteArray(), p ) {}
              ShareSource( const ShareSource &r )
                : DataVarLenLong( AO_SHARE_SOURCE, QByteArray(), r.parent() ),
                  amount( r.amount ), giverId( r.giverId ), page( r.page ), block( r.block ) {}
  QByteArray  toDataItem() { return QByteArray(); }

private:
    Shares amount;
    PubKey giverId;
   PageRef page;
  BlockRef block;
};

class ShareReceiver : public DataVarLenLong
{
    Q_OBJECT
public:
    explicit  ShareReceiver( QObject *p = nullptr) : DataVarLenLong( AO_SHARE_RECEIVER, QByteArray(), p ) {}
              ShareReceiver( const ShareReceiver &r ) : DataVarLenLong( AO_SHARE_RECEIVER, QByteArray(), r.parent() ),
                amount( r.amount ), receiverId( r.receiverId ) {}
  QByteArray  toDataItem() { return QByteArray(); }

private:
    Shares  amount;
    PubKey  receiverId;
};

class Transaction : public QObject
{
    Q_OBJECT
public:
    explicit  Transaction(QObject *parent = nullptr);
      AOTime  proposalTime();
        void  randomizeSalt();
        bool  valid();
        bool  validSum();
        bool  validTimeline();
  QByteArray  toByteArray() { return QByteArray(); }

private:
             Random  rng;
         QByteArray  salt;
            PageRef  proposedChain;          // Reference to the signature page of a recent block in the chain this transaction is proposed to be recorded on
             AOTime  preRecordingDeadline;   // Multi-part contracts may file pre-records to establish that all parts have been recorded before finalizing actual recording (this field is not present for simple contracts)
             AOTime  finalRecordingDeadline; // When the final record is expected to be recorded in the chain
             Shares  recordingBid;           // Positive amount to bid for all underwriting, chain-making and recording taxes
  QList<Participant> participants;
  QList<ShareSource> sources;
};

class Signature : public QObject
{
    Q_OBJECT
public:
    explicit  Signature( AOTime t, QObject *parent = nullptr)
        : QObject( parent ), timeOfSignature( t ) {}
  QByteArray  toByteArray() { return QByteArray(); }

private:
      AOTime timeOfSignature;
  QByteArray signature;
};

/**
 * @brief The Authorization class - when
 *   complete and valid, contains a description of
 *   the basic transaction between all the participants
 *   without the underwriting and recording - only the RBID is specified
 *   which describes the maximum commission payable to
 *   the sum of all underwriters, chain-maker, and recording tax.
 */
class Authorization : public QObject
{
    Q_OBJECT
public:
    explicit Authorization( QObject *parent = nullptr) : QObject( parent ) {}
             Authorization( Transaction t, QObject *parent = nullptr);
 QByteArray  toByteArray() { return QByteArray(); }

private:
      Transaction  tran;
  QList<Signature> sig;  // Same length and order as the participants list in tran
};

/* A structure to hold:
 *   <TRAN> Coin transfer transaction proposal:
 *     [SALT] 256 bit random number included in all signatures
 *     [TPRO] "now" from Alice's Wallet clock
 *     [TRMX] "now"+6 hours (closing deadline)
 *     [RBID] 3 coin (maximum recording fee)
 *     <PART> List of each participant's:
 *       [KEY]  Alice's "504" public key
 *       [AMT]  -504.023507972
 *       [NOTE] (empty)
 *       <PAGE> giver's source of funds
 *         [TREC] recording time of PAGE
 *         [HASH] hash of PAGE (found in BLOCK at TREC)
 *
 *       [KEY]  Alice's first new public key
 *       [AMT]  100
 *       [MINU] 100 (minimum acceptable underwriting amount to be countersigned by this key)
 *       [NOTE] (empty)
 *
 *       [KEY]  Alice's second new public key
 *       [AMT]  100
 *       [MINU] 100 (minimum acceptable underwriting amount)
 *       [NOTE] (empty)
 *
 *       [KEY]  Alice's third new public key
 *       [AMT]  100
 *       [MINU] 100 (minimum acceptable underwriting amount)
 *       [NOTE] (empty)
 *
 *       [KEY]  Alice's fourth new public key
 *       [AMT]  100
 *       [MINU] 100 (minimum acceptable underwriting amount)
 *       [NOTE] (empty)
 *
 *       [KEY]  Alice's fifth new public key
 *       [AMT]  96.023507972
 *       [MINU] 96.023507972 (minimum acceptable underwriting amount)
 *       [NOTE] (empty)
 *
 *       [KEY]  Bob's new public key
 *       [AMT]  5
 *       [MINU] 5 (minimum acceptable underwriting amount)
 *       [NOTE] Bob, you're weird.
 *
 * <AUTH> A fully described and authorized transaction:
 *   <PART> List of each participant's:
 *     [TSIG] time of signing (approving) the proposed transaction <TRAN>
 *     [SIG]  signature on: TRAN+AUTH(PART(TSIG)) using corresponding private TRAN(PART(KEY))
 *   <TRAN> as above, now wrapped in the AUTH structure
 *
 * AUTH(PART) and AUTH(TRAN(PART)) lists must be the same length and in the same order
 *
 * PAGE is only needed for givers, and is not necessary in initial transaction
 *   proposal, but must be accurately recorded before AUTH is signed
 * Perhaps: when MINU is absent, it is assumed to be AMT (by rules of the Genesis block)
 *
 */

#endif // TRANSACTION_H
