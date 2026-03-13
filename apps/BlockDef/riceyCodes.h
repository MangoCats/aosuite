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
#ifndef RICEYCODES_H
#define RICEYCODES_H
/*
List ordering: every object list is sorted by the RiceyInt key values.
Duplicate keys are not permitted in any object.
Arrays are ordered as created.
*/

#include "codeDef.h"
#include <QByteArray>
typedef QByteArray RiceyCode;
typedef QByteArray JsonSerial;
typedef QByteArray  BaoSerial;
typedef QByteArray Utf8String;
typedef QByteArray DotSerial;
typedef    quint64 RiceyInt;

     bool validRicey( const RiceyCode & );
RiceyCode intToRice( const RiceyInt & );
 RiceyInt riceToInt( const RiceyCode &, qint32 *len = nullptr, bool *ok = nullptr );

#define MAX_RICEY_LEN 9   // 63 bits
// TODO: MAX_MPINT_LEN might be better implemented as a protocol defined constant
#define MAX_MPINT_LEN 500 // base 10 digits, including negative sign

#endif // RICEYCODES_H
