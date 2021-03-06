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
#ifndef UNDERWRITING_H
#define UNDERWRITING_H

#include <QObject>
#include "authorization.h"

/* An Underwriting & recording data structure
 * <UWT> Underwritten Transaction, ready to record as a page in a block in the chain
 *   <AUTH> a proposed transaction, complete, valid and authorized by all participants
 *   <UWRP> Underwriting/Recording proposal:
 *     <UWR> List of each underwriter/recorder's:
 *       <DESC> Description of underwriting
 *         <KEYS> List of one or more keys that this underwriting applies to
 *           [KEY] an AUTH(TRAN(PART(KEY))) that this underwriting applies to
 *         [ADDR] current contact address (ipv4 or ipv6 or domain name, plus port and folder/filename)
 *         [FEE]  maximum amount to be taken by this underwriter as underwriting+recording fee, sum of all UWRP(UWR(DESC(FEE))) + tax <= AUTH(TRAN(RBID))
 *         [UWRK] public key of the underwriter/recorder (matches key on PAGE)
 *         <PAGE> previous (most recent) blockchain/page entry for UWRP(UWR(KEY)) - the whole value of this PAGE/UWRK is posted as underwriting
 *           [TREC] time of UWRP(UWR(PAGE))
 *           [HASH] hash of UWRP(UWR(PAGE))
 *       [TSIG] time of signing [TSIG]
 *       [SIG]  signature on: AUTH+UWRP(UWR(DESC)+TSIG) by private UWRK indicating acceptance of proposal by underwriter
 *     List continues until all AUTH(TRAN(PART(KEY))) are sufficiently underwritten by underwriters which the TRAN(PART(KEY)) holders will approve
 *     <UWRA> Underwriting/Recording approval List of signatures by all TRAN(PART(KEY)) private keys - list must be the same length and in the same order as AUTH(TRAN(PART)):
 *       [TSIG] time of signing [TSIG]
 *       [SIG]  signature on: AUTH+UWRP by private KEY indicating acceptance of the underwriting proposal package
 *
 *   <RCRD> Recording window - underwriters may negotiate amongst themselves a narrower recording window than is implied by
 *          the time limits of UWRA(TSIG) and AUTH(TRAN(TRMX))
 *     [TMIN] earliest authorized recording time >= last UWRP(UWRA(TSIG))
 *     [TMAX] latest authorized recording time <= AUTH(TRAN(TRMX))
 *
 *   <UWRV> Underwriting/Recording validation - list same length and order as <UWRP(UWR(DESC(UWRK)))
 *     [TSIG] time of signing [TSIG]
 *     [SIG]  signature on: UWRP+UWRV(TSIG) by private UWRK indicating validation of TRAN
 *
 * Note: unrecorded negotiation between participants determines what is an acceptable
 *   underwriting arrangement for all parties.  If any party to the transaction is not
 *   happy with the underwriting (usually recipients concerned about their own portions)
 *   then they do not sign the underwriting/recording proposal.  Either a new underwriting
 *   and recording proposal can be negotiated and signed, or the transaction is aborted.
 *
 *   Underwriters indicate their willingness to research the proposed transaction by their
 *   signature on the UWRP, but they may not perform the actual research and validation
 *   of the transaction until they receive a UWRP fully signed by all parties in the
 *   transaction.
 *
 * A page, ready for recording in a block, will include:
 *   Valid, fully completed and signed by all parties:
 *     Authorization
 *     Underwriting Proposal
 */

class Underwriting : public QObject
{
    Q_OBJECT
public:
    explicit Underwriting(QObject *p = NULL);

private:
    Authorization auth;
};

#endif // UNDERWRITING_H
