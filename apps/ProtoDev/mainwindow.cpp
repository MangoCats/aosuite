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
#include "mainwindow.h"
#include "ui_mainwindow.h"
#include <QSettings>
#include <QJsonArray>
#include <QJsonDocument>
#include <QFile>
#include "protocolDemo.h"
#include "riceyCodes.h"
#include "tests.h"
#include "aboutform.h"

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(new Ui::MainWindow)
{
    ui->setupUi(this);
    restoreConfig();
                new AboutForm   ( ui->    aboutTab );
    blockTool = new BlockTool   ( ui->blockToolTab );
           pd = new ProtocolDemo( ui->     demoTab );
                new Tests       ( ui->    testsTab );
    on_update_clicked();
    on_readJson_clicked();
    on_reformat_clicked();
}

MainWindow::~MainWindow()
{
    delete ui;
}

void MainWindow::closeEvent(QCloseEvent *event)
{ if ( blockTool )
    blockTool->closing();
  QSettings settings;
  settings.setValue( "geometry", saveGeometry() );
  settings.setValue( "state"   , saveState()    );
  QMainWindow::closeEvent(event);
}

/**
 * @brief MainWinCommon::restoreConfig - places the window at
 *   the same location and size on the desktop as when it was
 *   last closed.
 */
void MainWindow::restoreConfig()
{ QSettings settings;
  restoreGeometry( settings.value( "geometry" ).toByteArray() );
  restoreState   ( settings.value( "state"    ).toByteArray() );
  if ( settings.value( "ricey"       ).toString().size() > 0 ) ui->ricey      ->setHtml( settings.value( "ricey"       ).toString() );
  if ( settings.value( "notes"       ).toString().size() > 0 ) ui->notes      ->setHtml( settings.value( "notes"       ).toString() );
  if ( settings.value( "jsonPath"    ).toString().size() > 0 ) ui->jsonPath   ->setText( settings.value( "jsonPath"    ).toString() );
  if ( settings.value( "codeDefPath" ).toString().size() > 0 ) ui->codeDefPath->setText( settings.value( "codeDefPath" ).toString() );
}

/**
 * @brief MainWinCommon::saveConfig - saves the window location
 *   and size on the desktop to be restored on next launch.
 */
void MainWindow::saveConfig()
{ QSettings settings;
  settings.setValue( "ricey"      , ui->ricey   ->toHtml()  );
  settings.setValue( "notes"      , ui->notes   ->toHtml()  );
  settings.setValue( "jsonPath"   , ui->jsonPath->text()    );
  settings.setValue( "codeDefPath", ui->codeDefPath->text() );
}

/**
 * @brief MainWindow::on_importJson_clicked - the commonly used full sequence after a git pull with new definitions in .json
 */
void MainWindow::on_importJson_clicked()
{ on_readJson_clicked();
  on_save_clicked();
  on_update_clicked();
  on_write_clicked();
  on_reformat_clicked();
  on_save_clicked();
}

/**
 * @brief MainWindow::on_save_clicked - just save the configuration to the .ini file
 */
void MainWindow::on_save_clicked()
{ saveConfig(); }

/**
 * @brief MainWindow::on_update_clicked - translate the configuration to new .h and .json files
 */
void MainWindow::on_update_clicked()
{ init();
  firstPass();

  translateNotes();
  t.append( "\n" );
  translateRicey();

  translateToJson();

  showResults();
}

/**
 * @brief MainWindow::on_readJson_clicked - reads the .json resource and spits it into the configuration, unformatted but suitable for reformat
 */
void MainWindow::on_readJson_clicked()
{ QFile f(":/data/dictionary.json");
  if ( !f.open( QIODevice::ReadOnly ) )
    { qWarning( "cannot open resource file /data/dictionary.json" );
      return;
    }
  QJsonDocument jd = QJsonDocument::fromJson( f.readAll() );
  if ( !jd.isObject() )
    { qWarning( "resource dictionary is not a json object" );
      return;
    }
  QJsonObject jo = jd.object();
  if ( !jo.contains( "riceyTypes_O" ) )
    { qWarning( "resource dictionary does not contain types definition" );
      return;
    }
  if ( !jo.contains( "riceyCodes_O" ) )
    { qWarning( "resource dictionary does not contain codes definition" );
      return;
    }
  if ( !jo["riceyTypes_O"].isArray() )
    { qWarning( "resource dictionary types not in array form" );
      return;
    }
  if ( !jo["riceyCodes_O"].isArray() )
    { qWarning( "resource dictionary codes not in array form" );
      return;
    }
  QJsonArray ca = jo["riceyCodes_O"].toArray();
  qint32 sz = ca.size();
  QString codes;
  for ( qint32 i = 0; i < sz; i++ )
    { QJsonValue jv = ca[i];
      if ( !jv.isObject() )
        qWarning( "code %d is not an object", i );
       else
        { QJsonObject cao = jv.toObject();
          QString name,typ,group,desc; desc.reserve( 64 );
          if ( cao.contains( "name_s" ) )
            name =       cao["name_s"].toString();
          if ( cao.contains( "group_s" ) )
            group =      cao["group_s"].toString();
          if ( cao.contains( "text_s" ) )
            desc =       cao["text_s"].toString().trimmed();
          if ( cao.contains( "type_b" ) )
            typ =        cao["type_b"].toString();
          if ( !desc.startsWith( group ) )
            desc.prepend( group.trimmed()+" " );
          codes.append( QString( "%1 0x%2 // %3\n").arg( name, typ, desc ) );
        }
    }
  if ( codes.size() > 0 )
    ui->ricey->setPlainText( codes );

  QJsonArray ta = jo["riceyTypes_O"].toArray();
  sz = ta.size();
  QString types;
  for ( qint32 i = 0; i < sz; i++ )
    { QJsonValue jv = ta[i];
      if ( !jv.isObject() )
        qWarning( "type %d is not an object", i );
       else
        { QJsonObject tao = jv.toObject();
          QString name,typ,desc,code;
          if ( tao.contains( "code_s" ) )
            code =       tao["code_s"].toString();
          if ( tao.contains( "name_s" ) )
            name =       tao["name_s"].toString();
          if ( tao.contains( "text_s" ) )
            desc =       tao["text_s"].toString();
          if ( tao.contains( "type_b" ) )
            typ =        tao["type_b"].toString();
          types.append( QString( "%1 0x%2 // %3 - %4\n").arg( name, typ, code, desc ) );
        }
    }
  if ( types.size() > 0 )
    ui->notes->setPlainText( types );

}

/**
 * @brief MainWindow::on_write_clicked - writes the codeDef.h and dictionary.json files
 */
void MainWindow::on_write_clicked()
{ QFile file( ui->jsonPath->text() );
  if ( !file.open( QIODevice::WriteOnly ) )
    { ui->translations->append( QString( "Could not open %1 for writing.\n" ).arg( ui->jsonPath->text() ) );
      return;
    }
  file.write( ui->json->toPlainText().toUtf8() );

  QFile cFile( ui->codeDefPath->text() );
  if ( !cFile.open( QIODevice::WriteOnly ) )
    { ui->translations->append( QString( "Could not open %1 for writing.\n" ).arg( ui->codeDefPath->text() ) );
      return;
    }
  cFile.write( ui->translations->toPlainText().toUtf8() );
}

/**
 * @brief MainWindow::init - reset everything ready for a new translation
 */
void MainWindow::init()
{ v.clear();
  t.clear();
  notesNumChar.clear();
  notesExtraChar.clear();
  notesList = ui->notes->toPlainText().split("\n",Qt::SkipEmptyParts);
  riceyList = ui->ricey->toPlainText().split("\n",Qt::SkipEmptyParts);
  maxNameLength = 0;
  maxNumLength  = 0;
  maxCombLength = 0;
}

/**
 * @brief MainWindow::showResults - update the translations and json windows
 */
void MainWindow::showResults()
{ ui->translations->clear();
  ui->translations->append( "/* MIT License" );
  ui->translations->append( " *" );
  ui->translations->append( " * Copyright (c) 2021 Assign Onward" );
  ui->translations->append( " *" );
  ui->translations->append( " * Permission is hereby granted, free of charge, to any person obtaining a copy" );
  ui->translations->append( " * of this software and associated documentation files (the \"Software\"), to deal" );
  ui->translations->append( " * in the Software without restriction, including without limitation the rights" );
  ui->translations->append( " * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell" );
  ui->translations->append( " * copies of the Software, and to permit persons to whom the Software is" );
  ui->translations->append( " * furnished to do so, subject to the following conditions:" );
  ui->translations->append( " *" );
  ui->translations->append( " * The above copyright notice and this permission notice shall be included in all" );
  ui->translations->append( " * copies or substantial portions of the Software." );
  ui->translations->append( " *" );
  ui->translations->append( " * THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR" );
  ui->translations->append( " * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY," );
  ui->translations->append( " * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE" );
  ui->translations->append( " * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER" );
  ui->translations->append( " * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM," );
  ui->translations->append( " * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE" );
  ui->translations->append( " * SOFTWARE.");
  ui->translations->append( " */");
  ui->translations->append( "#ifndef CODEDEF_H" );
  ui->translations->append( "#define CODEDEF_H" );
  ui->translations->append( "" );
  if ( v.size() > 0 )
    t.append( "\n"+v );
  ui->translations->append( t );
  ui->translations->append( "" );
  ui->translations->append( "#endif // CODEDEF_H" );

  ui->json->clear();
  QJsonDocument jd = QJsonDocument( jo );
  ui->json->append( QString::fromUtf8( jd.toJson() ) );
}

/**
 * @brief MainWindow::firstPass - pick up some easy stuff for use in the later passes
 */
void MainWindow::firstPass()
{ QVector<QString> names;
  QVector<QString> codes;
  riceyInts.clear();
  riceyCodes.clear();
  foreach ( QString line, riceyList )
    { QVector<QString> words = line.split(" ",Qt::SkipEmptyParts);
      if ( words.size() > 1 )
        { if ( words.at(0).size() > maxNameLength )
            maxNameLength = words.at(0).size();
          if ( names.contains( words.at(0) ) )
            { v.append( QString( "ERROR: name %1 used twice, second time in line '%2'\n" ).arg( words.at(0), line ) ); }
           else
            names.append( words.at(0) );

          riceyCodes.insert( words.at(0), words.at(1) );

          if ( !words.at(1).startsWith( "0x" ) || (words.at(1).size() < 3) )
            { v.append( QString( "ERROR: num '%1' must start with 0x and be followed by at least 1 digit in line '%2'\n" ).arg( words.at(1), line ) ); }
           else
            { RiceyInt nv = riceToInt( QByteArray::fromHex( words.at(1).mid(2).toUtf8() ) );
              if ( riceyInts.contains( nv ) )
                { v.append( QString( "ERROR: RiceyInt value collision, duplicate int form in line '%1'\n" ).arg( line ) ); }
               else
                riceyInts.append( nv );
              if ( intToRice( nv ) != QByteArray::fromHex( words.at(1).mid(2).toUtf8() ) )
                { v.append( QString( "ERROR: code '%1' reconstitutes as '%2' in line '%3'\n" ).arg(words.at(1).mid(2), QString::fromUtf8( intToRice(nv).toHex() ), line ) ); }
              // qDebug( "words.at(1) %s -> %lld", words.at(1).toUtf8().data(), nv );
              qint32 sz = QString::number( nv ).size();
              if ( sz > maxNumLength )
                maxNumLength = sz;
              if ( (words.at(0).size() + words.at(1).size()) > maxCombLength )
                maxCombLength = words.at(0).size() + words.at(1).size();
              if ( codes.contains( words.at(1) ) )
                { v.append( QString( "ERROR: code %1 used twice, second time in line '%2'\n" ).arg( words.at(1), line ) ); }
               else
                codes.append( words.at(1) );
            }
        }
       else
        { v.append( QString( "WARN: short line '%1' in Ricey Code Definitions.\n" ).arg( line ) );
        }
    }
  bool ok;
  foreach ( QString line, notesList )
    { QVector<QString> words = line.split(" ",Qt::SkipEmptyParts);
      if ( words.size() > 3 )
        { if ( words.at(1).startsWith( "0x" ) )
            { if (( words.at(1).size() > 2 ) && ( words.at(1).size() < 5 ))
                { if ( words.at(3).size() == 1 )
                    { if ( notesNumChar.contains( words.at(1).toInt(&ok,16) & RDT_OBTYPEMASK ) )
                        notesExtraChar.insert( words.at(1).at(2), words.at(3).at(0) );
                       else
                        notesNumChar.insert( words.at(1).toInt(&ok,16) & RDT_OBTYPEMASK, words.at(3).at(0) );
                    } else { v.append( QString( "WARN: 4th word in Code Notes '%1' isn't length 1.\n" ).arg( line ) ); }
                } else { v.append( QString( "WARN: 2nd word in Code Notes '%1' isn't length 2 or 3.\n" ).arg( line ) ); }
            } else { v.append( QString( "WARN: 2nd word in Code Notes '%1' doesn't start with 0x.\n" ).arg( line ) ); }
        } else { v.append( QString( "WARN: line '%1' in Code Notes doesn't have 4 or more words.\n" ).arg( line ) ); }
    }
}

/**
 * @brief MainWindow::translateRicey - also check for correctness as we go
 */
void MainWindow::translateRicey()
{ QString rPre = ui->riceyPrefix->text();
  foreach ( QString line, riceyList )
    { qint32 ics = line.indexOf( "//" );
      if ( ics < 0 )
        v.append( "WARN: missing // in: "+line+"\n" );
       else
        { QString left = line.mid( 0, ics );
          QVector<QString> lList = left.split(" ",Qt::SkipEmptyParts);
          if ( lList.size() != 2 )
            v.append( QString( "WARN: %1 words before // in: %2\n" ).arg( lList.size() ).arg( line ) );
          if ( lList.size() > 1 )
            { if ( !rulesCheck( lList.at(0), lList.at(1) ) )
                v.append( QString( "WARN: rules check failed in: %1\n" ).arg( line ) );
               else
                { // Temporary output for warm fuzzies
                  QString name = lList.at(0);
                  while ( name.size() < maxNameLength )
                    name.append( QChar(' ') );
                  QString num; num.reserve( maxNumLength + 1 );
                  num = QString::number( riceToInt( QByteArray::fromHex( lList.at(1).mid(2).toUtf8() ) ) );
                  while ( num.size() < maxNumLength )
                    num.prepend( QChar(' ') );
                  t.append( QString( "#define %4%1 %2 %3\n" ).arg( name, num, line.mid(ics), rPre ) );
                }
            }
        }
    }
}


/**
 * @brief MainWindow::rulesCheck - operating on the ricey code definitions lines
 * @param name - code name
 * @param num  - code number
 * @return true if the rules are met
 */
bool MainWindow::rulesCheck( QString name, QString num )
{ if ( name.size() < 2 )
    { v.append( QString( "rulesCheck: name '%1' too short.\n" ).arg( name ) );
      return false;
    }
  if ( num.size() < 4 )
    { v.append( QString( "rulesCheck: num '%1' too short.\n" ).arg( num ) );
      return false;
    }
  if ( !num.startsWith( "0x" ) )
    { v.append( QString( "rulesCheck: num '%1' does not start with 0x.\n" ).arg( num ) );
      return false;
    }
  bool ok;
  qint64 numlc = riceToInt( QByteArray::fromHex( num.mid(2).toUtf8() ), nullptr, &ok ) & RDT_OBTYPEMASK;
  if ( !ok )
    { v.append( QString( "rulesCheck: num '%1' did not successfully convert riceToInt().\n" ).arg( num ) );
      return false;
    }
  if ( !notesNumChar.contains( numlc ) )
    { v.append( QString( "rulesCheck: num '%1' does not end with a recognized code.\n" ).arg( num ) );
      return false;
    }
  QChar namelc = name.at(name.size()-1);
  if ( notesNumChar[numlc] != namelc ) // A kind of post-fix Hungarian notation to make the json/bao strictly typed
    { v.append( QString( "rulesCheck: name '%1' does not end '%2' with the expected %3 %4 character %4.\n" )
                   .arg( name ).arg( namelc ).arg( numlc ).arg( num )
                   .arg( notesNumChar[numlc] ) );
      return false;
    }
  QByteArray ba = QByteArray::fromHex( num.mid(2).toUtf8() );
  if ( ba.size() < 1 )
    { v.append( QString( "rulesCheck: '%1' does not translate as bytes of hex code.\n" ).arg( num ) );
      return false;
    }
  if ( ba.size() > MAX_RICEY_LEN )
    { v.append( QString( "rulesCheck: '%1' too long for an AO Rice Code (%2 bytes, max is %3).\n" ).arg( num ).arg( ba.size() ).arg( MAX_RICEY_LEN ) );
      return false;
    }
  if ( !validRicey( ba ) )
    { v.append( QString( "rulesCheck: '%1' is not a valid Rice code, all bytes except the last must set the 0x80 bit.\n" ).arg( num ) );
      return false;
    }
  return true;
}

/**
 * @brief MainWindow::translateNotes - write the .h code defines for the notes (Ricey Data Types)
 */
void MainWindow::translateNotes()
{ QString nPre = ui->notesPrefix->text();
  foreach ( QString line, notesList )
    { t.append( QString( "#define %2%1\n" ).arg(line, nPre) );
    }
}

/**
 * @brief MainWindow::translateToJson - make a json object that contains the content of the ricey and notes windows
 */
void MainWindow::translateToJson()
{ jo = QJsonObject();
  QJsonArray ja;
  foreach ( QString line, riceyList )
    ja.append( riceyLineToJson( line ) );
  jo.insert( "riceyCodes_O", ja );
  QJsonArray jt;
  foreach ( QString line, notesList )
    jt.append( notesLineToJson( line ) );
  jo.insert( "riceyTypes_O", jt );
}

QJsonValue MainWindow::riceyLineToJson( QString line )
{ QJsonObject rlo;
  QVector<QString> words = line.split( QChar(' '), Qt::SkipEmptyParts );
  if ( words.size() < 4 )
    { v.append( QString( "ricey line '%1' doesn't have at least 4 words" ).arg( line ) ); }
   else
    { rlo.insert( "name_s", words.at(0) );
      if ( words.at(1).size() < 3 )
        { v.append( QString( "ricey line '%1' code '%2' too short" ).arg( line, words.at(1) ) ); }
       else if ( !words.at(1).startsWith( "0x" ) )
        { v.append( QString( "ricey line '%1' code '%2' doesn't start with 0x" ).arg( line, words.at(1) ) ); }
       else
        { rlo.insert( "type_b", words.at(1).mid(2) );
        }
      if ( words.at(2) != "//" )
        { v.append( QString( "ricey line '%1' doesn't have // as 3rd word" ).arg( line ) ); }
       else
        { if ( words.size() == 3 )
            rlo.insert( "text_s", line.mid( line.indexOf("//") + 3 ) );
           else
            rlo.insert( "text_s", line.mid( line.indexOf("//") + 4 + words.at(3).size() ) );
        }
      if ( words.size() > 3 )
        rlo.insert( "group_s", words.at(3) );
    }
  return QJsonValue( rlo );
}

QJsonValue MainWindow::notesLineToJson( QString line )
{ QJsonObject nlo;
   QVector<QString> words = line.split( QChar(' '), Qt::SkipEmptyParts );
  if ( words.size() < 4 )
    { v.append( QString( "notes line '%1' doesn't have at least 4 words" ).arg( line ) ); }
   else
    { nlo.insert( "name_s", words.at(0) );
      if ( words.at(1).size() < 2 )
        { v.append( QString( "notes line '%1' code '%2' too short" ).arg( line, words.at(1) ) ); }
       else if ( !words.at(1).startsWith( "0x" ) )
        { v.append( QString( "notes line '%1' code '%2' doesn't start with 0x" ).arg( line, words.at(1) ) ); }
       else
        { nlo.insert( "type_b", words.at(1).mid(2) );
        }
      if ( words.at(2) != "//" )
        { v.append( QString( "notes line '%1' doesn't have // as 3rd word" ).arg( line ) ); }
       else
        { nlo.insert( "text_s", line.mid( line.indexOf("//") + 7 ) ); }
      nlo.insert( "code_s", words.at(3) );
    }
  return QJsonValue( nlo );
}

/**
 * @brief MainWindow::on_reformat_clicked - reconstruct the input lists in consistent alignment and color
 */
void MainWindow::on_reformat_clicked()
{ QString html;
  int maxLen = 0;
  foreach ( QString line, notesList )
    { QVector<QString> words = line.split( QChar(' '), Qt::SkipEmptyParts );
      if ( words.size() > 0 )
        if ( words.at(0).size() > maxLen )
          maxLen = words.at(0).size();
    }
  foreach ( QString line, notesList )
    html.append( notesLineToHtml( line, maxLen ) );
  ui->notes->clear();
  ui->notes->setHtml( html );

  html.clear();
  foreach ( QString line, riceyList )
    html.append( codesLineToHtml( line ) );
  ui->ricey->clear();
  ui->ricey->setHtml( html );
}

QString MainWindow::notesLineToHtml( QString line, int maxLen )
{ QString html = "<font color=#000000>";
  QVector<QString> words = line.split( QChar(' '), Qt::SkipEmptyParts );
  if ( words.size() < 4 )
    { v.append( QString( "notes line '%1' doesn't have at least 4 words" ).arg( line ) ); }
   else
    { QString word = words.at(0);
      int sz = word.size();
      for ( int i = 0; i < maxLen - sz; i++ )
        word.append( "&nbsp;" );
      html.append( word+"</font><font color=#000080> "+words.at(1)+" </font><font color=#008000> // "+words.at(3) );
      if ( words.at(2) != "//" )
        { v.append( QString( "notes line '%1' doesn't have // as 3rd word" ).arg( line ) ); }
       else
        { html.append( " - "+line.mid( line.indexOf("//") + 7 ) ); }
    }
  html.append( "</font><br/>" );
  return html;
}

QString MainWindow::codesLineToHtml( QString line )
{ QString html = "<font color=#000000>";
  QVector<QString> words = line.split( QChar(' '), Qt::SkipEmptyParts );
  if ( words.size() < 4 )
    { v.append( QString( "notes line '%1' doesn't have at least 4 words" ).arg( line ) ); }
   else
    { QString word = words.at(0);
      int sz = word.size() + words.at(1).size() + 1;
      for ( int i = 0; i < maxCombLength - sz + 2; i++ )
        word.append( "&nbsp;" );
      html.append( word+"</font><font color=#000080>"+words.at(1)+" </font><font color=#008000> // " );
      if ( words.at(2) != "//" )
        { v.append( QString( "codes line '%1' doesn't have // as 3rd word" ).arg( line ) ); }
       else
        { QString desc = line.mid( line.indexOf("//") + 2 );
          html.append( desc );
        }
    }
  html.append( "</font><br/>" );
  return html;
}
