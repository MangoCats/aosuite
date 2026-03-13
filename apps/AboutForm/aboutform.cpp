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
#include "aboutform.h"
#include "appname.h"

AboutForm::AboutForm( QWidget *cw ) :
    QScrollArea(cw),
    ui(new Ui::AboutForm)
{ ui->setupUi(this);
  new QVBoxLayout( cw );
  cw->layout()->addWidget( this );
  ui->appName   ->setText( APPNAME    );
  ui->appDesc   ->setText( APPDESC    );
  ui->appVersion->setText( APPVERSION );
  ui->appStatus ->setText( APPSTATUS  );
  showLibVersions();
}

AboutForm::~AboutForm()
{ delete ui;
}

#include <gmp.h>
#include <gcrypt.h>

void AboutForm::showLibVersions()
{ ui->libVersions->append( QString( "GMP %1"      ).arg( gmp_version                                   ) );
  ui->libVersions->append( QString( "libgcrypt %1").arg( QString::fromUtf8( gcry_check_version(NULL) ) ) );
  ui->libVersions->append( QString( "Qt %1"       ).arg( QString::fromUtf8( qVersion()               ) ) );
}
