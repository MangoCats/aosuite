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
#ifndef RANDOM_H
#define RANDOM_H
#include <stdint.h>
#include <QByteArray>
#include <QStringList>

class Random
{
public:
               Random( __int128 seed = 42 );
         void  seed( __int128 seed );
     uint64_t  rnd_uint64();
      int64_t  rnd_int64();
         bool  rnd_bool();
   QByteArray  rnd_bytes( qint32 n = 4 );
        float  rnd_float( float min, float max );
  QStringList  rnd_nameAndEmail();
         void  initializeNames();

private:
    /* The state must be seeded so that it is not all zero */
     uint64_t  s[2];
  QStringList  maleNames;
  QStringList  femaleNames;
  QStringList  lastNames;
  QStringList  emailDomains;
         bool  namesInitialized;
};

extern Random  rng; // Global object

#endif // RANDOM_H
