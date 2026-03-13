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
#include <QtWidgets>
#include "blockTool.h"
#include "ui_blockTool.h"
#include "blockOb.h"
#include <unistd.h>

BlockTool::BlockTool( QWidget *cw ) :
    QScrollArea(cw),
    ui(new Ui::BlockTool)
{ swapping = false;
  ui->setupUi(this);
  ui->remove->setStyleSheet( buttonStyle( ValueBase::Mode::build ) );
  ui->insert->setStyleSheet( buttonStyle( ValueBase::Mode::build ) );
  ui->set   ->setStyleSheet( buttonStyle( ValueBase::Mode::build ) );
  if ( cw )
    { QVBoxLayout *vb = new QVBoxLayout( cw );
      cw->layout()->addWidget( this );
      vb->setContentsMargins( 0,0,0,0 );
    }
  // ui->navGroup->setEnabled( false );
  panelA = new BlockPanel( "A", ValueBase::Mode::build, ui->frameA );
  panelX = new BlockPanel( "X", ValueBase::Mode::make , ui->frameX );
  panelY = new BlockPanel( "Y", ValueBase::Mode::idle , ui->frameY );
  connect( this, SIGNAL(showA(KeyValueBase*)), panelA, SLOT(setKeyValueBlock(KeyValueBase*)));
  connect( this, SIGNAL(showX(KeyValueBase*)), panelX, SLOT(setKeyValueBlock(KeyValueBase*)));
  connect( this, SIGNAL(showY(KeyValueBase*)), panelY, SLOT(setKeyValueBlock(KeyValueBase*)));
  ui->splitter->setStretchFactor( 0, 1 );
  ui->splitter->setStretchFactor( 1, 1 );
  initReadFile();
  sortKeys();
  restore();
  ui->sortDict->setChecked( true );
}

BlockTool::~BlockTool()
{ delete ui; }

void  BlockTool::closing()
{ QSettings settings;
  BaoSerial s;
  if ( panelA != nullptr )
    if ( panelA->m_kvb != nullptr )
      s = panelA->m_kvb->bao();
  settings.setValue( "panelA", s );

  s.clear();
  if ( panelX != nullptr )
    if ( panelX->m_kvb != nullptr )
      s = panelX->m_kvb->bao();
  settings.setValue( "panelX", s );

  s.clear();
  if ( panelY != nullptr )
    if ( panelY->m_kvb != nullptr )
      s = panelY->m_kvb->bao();
  settings.setValue( "panelY", s );
}

void  BlockTool::restore()
{ QSettings settings;
  KeyValueBase *kvb;
  BaoSerial s;
  s = settings.value( "panelA" ).toByteArray();
  if ( s.size() > 0 )
    { if ( panelA == nullptr )
        qWarning( "unexpected, panelA is nullptr" );
       else
        { kvb = KeyValueBase::readBao( s, panelA );
          if ( kvb == nullptr )
            qWarning( "bad read of panelA in restore()" );
           else
            { panelA->setKeyValueBlock( kvb, false );
              selectRoot( panelA );
              panelA->update();
            }
        }
    }
  s = settings.value( "panelX" ).toByteArray();
  if ( s.size() > 0 )
    { if ( panelX == nullptr )
        qWarning( "unexpected, panelX is nullptr" );
       else
        { kvb = KeyValueBase::readBao( s, panelX );
          if ( kvb == nullptr )
            qWarning( "bad read of panelX in restore()" );
           else
            { panelX->setKeyValueBlock( kvb, false );
              panelX->update();
            }
        }
    }
  s = settings.value( "panelY" ).toByteArray();
  if ( s.size() > 0 )
    { if ( panelY == nullptr )
        qWarning( "unexpected, panelY is nullptr" );
       else
        { kvb = KeyValueBase::readBao( s, panelY );
          if ( kvb == nullptr )
            qWarning( "bad read of panelY in restore()" );
           else
            { panelY->setKeyValueBlock( kvb, false );
              panelY->update();
            }
        }
    }
}

void  BlockTool::updateValueEditor()
{ if ( selBB == nullptr ) // No navigation point active
    { valueEditorNoNav();
      // qWarning( "selBB nullptr" );
      return;
    }
  //  qWarning( "updateValueEditor   type=%x isArray=%d isContainer=%d m_key=0x%llx m_idx=%s",
  //             selBB->type(),selBB->isArray(),selBB->isContainer(),selBB->vKey(),selBB->idx().data() );
  if ( (!selBB->isContainer()) || selBB->isArray() )
    editSelectedElement();
   else
    selectObjectForInsertion();
}

/**
 * @brief BlockTool::valueEditorNoNav - default configuration
 *   for when nothing is selected by the navigator.
 */
void  BlockTool::valueEditorNoNav()
{ // qWarning( "BlockTool::valueEditorNoNav()" );
  ui->index      ->setVisible( false );
  ui->key        ->setVisible( true  );
  ui->key        ->setEnabled( true  );
  ui->valueWidget->setVisible( true  );
  ui->valueWidget->setEnabled( true  );
  ui->set        ->setVisible( false );
  ui->insert     ->setVisible( true  );
  ui->insert     ->setEnabled( true  );
  ui->remove     ->setVisible( false );
}

void  BlockTool::selectObjectForInsertion()
{ ValueBase *vb = selBB;
  if ( vb == nullptr )
    { qWarning( "BlockTool::selectObjectForInsertion() needs a valid selBB" );
      valueEditorNoNav();
      return;
    }
//  qWarning( "selBB is an Object" );
  ui->remove->setVisible( true );
  if ( ui->navBuild->isChecked() )
    { ui->index      ->setVisible( false );
      ui->key        ->setVisible( false );
      ui->valueWidget->setVisible( false );
      ui->set        ->setVisible( false );
      ui->insert     ->setVisible( true  );
      ui->insert     ->setEnabled( makeKvb() != nullptr );
    }
}

/**
 * @brief BlockTool::editSelectedElement - an atomic value element is selected
 *   enable changing of that value with the set button
 */
void  BlockTool::editSelectedElement()
{ ValueBase *vb = selBB;
  if ( vb == nullptr )
    { qWarning( "BlockTool::editSelectedElement() needs a valid selBB" );
      valueEditorNoNav();
      return;
    }
  // Editing value of selected element
  RiceyInt k = vb->vKey();
  if (( k == RCD_null_z ) || ( !dict.codesContainCode( k ) ))
    { if ( k == RCD_null_z )
        { // qWarning( "BlockTool::editSelectedElement() null key" );
        }
       else
        qWarning( "BlockTool::editSelectedElement() unknown key %llu", k );
      valueEditorNoNav();
      return;
    }
  int i = ui->key->findText( dict.nameFromCode( k ) );
  if ( i < 0 )
    { Utf8String keyName = dict.nameFromCode( k );
      qWarning( "key %s not found in ui", keyName.data() );
      valueEditorNoNav();
      return;
    }
  //qWarning( "editSelectedElement type=%x isArray=%d isContainer=%d m_key=0x%llx m_idx=%s",
  //           vb->type(),vb->isArray(),vb->isContainer(),k,vb->idx().data() );
  ui->key->setVisible( true  );
  ui->key->setEnabled( false );
  ui->key->setCurrentIndex( i ); // this also selects the correct valueWidget page

  ui->index->setVisible( vb->isArray() );
  if ( vb->isArray() ) // set index value range to array current size
    ui->index->setMaximum( ((ValueBaseArray *)vb)->size() );
   else   // Why are we getting isArray for array elements?
    setEditorValue( vb );
  ui->valueWidget->setVisible( true );
  ui->valueWidget->setEnabled( true );
  ui->set        ->setVisible( !vb->isArray() );
  ui->set        ->setEnabled( true );
  ui->insert     ->setVisible(  vb->isArray() );
  ui->insert     ->setEnabled( true );
  ui->remove     ->setVisible( true );
}

/**
 * @brief BlockTool::setEditorValue - set the editor to show the current value of the selected object (atomic values)
 * @param vb - object to copy the current value from
 */
void  BlockTool::setEditorValue( ValueBase *vb )
{ qint32 i;
  switch ( vb->type() & RDT_TYPEMASK )
   { case RDT_INT64:     ui->      intEdit->setText( Utf8String::number((qobject_cast<BlockValueInt64    *>(vb))->value())); break;
     case RDT_RICEYINT:  ui-> riceyIntEdit->setText( Utf8String::number((qobject_cast<BlockValueRiceyInt *>(vb))->value())); break;
     case RDT_MPZ:       ui->      mpzEdit->setText( ValueBase::removeQuotes((qobject_cast<BlockValueMPZ *>(vb))->json()) ); break;
     case RDT_MPQ:       ui->      mpqEdit->setText( ValueBase::removeQuotes((qobject_cast<BlockValueMPQ *>(vb))->json()) ); break;
     case RDT_STRING:    ui->   stringEdit->setText( (qobject_cast<BlockValueString    *>(vb))->value()         ); break;
     case RDT_BYTEARRAY: ui->byteArrayEdit->setText( (qobject_cast<BlockValueByteArray *>(vb))->value().toHex() ); break;
     case RDT_RCODE:
       i = ui->rcodeEdit->findText( dict.nameFromCode( (qobject_cast<BlockValueRiceyCode *>(vb))->value()) );
       if ( i > 0 )
         ui->rcodeEdit->setCurrentIndex( i );
       break;
   }
}

/**
 * @brief BlockTool::on_set_clicked - set has been clicked, hopefully while one of the atomic
 *   value types is selected.  Update its value with that from the ui edit field, or throw a
 *   warning message if there is a problem.
 */
void  BlockTool::on_set_clicked()
{ ValueBase *vb = selBB;
  if ( vb == nullptr ) { qWarning( "attempt to set when nothing is selected" );                    return; }
  if ( vb->isArray() ) { qWarning( "shouldn't be able to set in an array element (only insert)" ); return; }
  Utf8String codeName;
  switch ( vb->type() & RDT_TYPEMASK )
    { case RDT_MPZ:       (qobject_cast<BlockValueMPZ       *>(vb))->set( ui->     mpzEdit->text().toUtf8()     ); updateNav(); break;
      case RDT_MPQ:       (qobject_cast<BlockValueMPQ       *>(vb))->set( ui->     mpqEdit->text().toUtf8()     ); updateNav(); break;
      case RDT_INT64:     (qobject_cast<BlockValueInt64     *>(vb))->set( ui->     intEdit->text().toLongLong() ); updateNav(); break;
      case RDT_RICEYINT:  (qobject_cast<BlockValueRiceyInt  *>(vb))->set( ui->riceyIntEdit->text().toLongLong() ); updateNav(); break;
      case RDT_STRING:    (qobject_cast<BlockValueString    *>(vb))->set( ui->  stringEdit->text().toUtf8()     ); updateNav(); break;
      case RDT_BYTEARRAY: (qobject_cast<BlockValueByteArray *>(vb))->set( QByteArray::fromHex( ui->byteArrayEdit->text().toUtf8() ) ); updateNav(); break;
      case RDT_RCODE:
        codeName = ui->rcodeEdit->currentText().toUtf8();
        if ( dict.codesContainName( codeName ) )
          { (qobject_cast<BlockValueRiceyCode *>(vb))->set( dict.codeFromCodeName( codeName ) );
            updateNav();
          }
         else
          qWarning( "dictionary does not contain %s", codeName.data() );
        break;
      default:
        qWarning( "BlockTool::on_set_clicked() unrecognized type %x", vb->type() & RDT_TYPEMASK );
    }
}

void  BlockTool::on_remove_clicked()
{ ValueBase *vb = selBB;
  if ( vb           == nullptr )     { qWarning( "BlockTool::on_remove_clicked() needs a valid selBB" );                              return; }
  if ( vb->vbParent == nullptr )     { qWarning( "BlockTool::on_remove_clicked() will not remove the root object, just use Clear." ); return; }
  if ( vb->vbParent->isKeyValue()  ) { qWarning( "BlockTool::on_remove_clicked() Cannot remove from root container, use Clear." );    return; }
  if (!vb->vbParent->isContainer() ) { qWarning( "BlockTool::on_remove_clicked() Cannot remove from non-containers" );                return; }
  on_prev_clicked();
  RiceyInt key = RCD_null_z;
  if ( vb->vbParent->isArray() )
    key = removeArrayElement( vb );
   else
    key = removeObjectElement( vb );
  if ( key == RCD_null_z ) { qWarning( "remove returned null_z key, can't complete removal." ); return; }
  if ( ui->navBuild->isChecked() )
    { on_clear_clicked();
      makePanel()->setKeyValueBlock( intToRice( key )+vb->bao() );
      updateBuild();
      vb->deleteLater();
    }
   else if ( ui->navMake->isChecked() )
    { vb->deleteLater();
      updateNav();
    }
   else
    qWarning( "neither make nor build checked in nav.");
}

RiceyInt BlockTool::removeArrayElement( ValueBase *vb )
{ if ( !vb->vbParent->isArray() ) { qWarning( "vb is not an array element, how did we get here?" ); return RCD_null_z; }
  ValueBaseArray *vba = qobject_cast<ValueBaseArray *>(vb->vbParent);
  qint32 i = vb->idx().mid(1).toInt();
  if (( i < 0 ) || ( i >= vba->size() )) { qWarning( "invalid index %d vs parent size %d", i, vba->size() ); return RCD_null_z; }
  if ( vb != vba->removeAt(i) )            qWarning( "odd, removed different object than selBB's idx was pointed at..." );
  return vb->vKey() & RDT_TYPEMASK;
}

RiceyInt BlockTool::removeObjectElement( ValueBase *vb )
{ if ( vb->vbParent->isArray() ) { qWarning( "vb is an array element, how did we get here?" ); return RCD_null_z; }
    BlockValueObject *pbvo = qobject_cast<BlockValueObject *>(vb->vbParent);
    RiceyInt key = vb->vKey();
    if ( !pbvo->contains( key ) )  { qWarning( "BlockTool::removeObjectElement() key 0x%llx not found in parent object", key ); return RCD_null_z; }
    if ( vb != pbvo->remove( key ) ) qWarning( "odd, removed different object than selBB was pointed at..." );
    return key;
}

void  BlockTool::on_insert_clicked()
{ if ( ui->navMake->isChecked() )
    { if ( makeKvb() == nullptr )
        { // New kvb in make register
          insertKeyName( ui->key->currentText().toUtf8() );
          updateValueEditor();
          ui->navGroup->setEnabled( true );
          return;
        }
    }
  if ( ui->navBuild->isChecked() )
    { if ( makeKvb() == nullptr ) { qWarning( "Make is empty"    ); return; }
      if ( selBB     == nullptr ) { qWarning( "Nothing selected" ); return; }
      RiceyInt key = makeKvb()->key();
      if ( selBB->type() == RDT_OBJECT )
        { BlockValueObject *bvo = qobject_cast<BlockValueObject *>(selBB);
          if ( bvo->contains(key) )
            { qWarning( "selected object already contains a member with type 0x%llx", key );
              return;
            }
          if ( makeKvb()->isKeyValuePair() )
            { KeyValuePair *kvp = qobject_cast<KeyValuePair *>(makeKvb());
              bvo->insert( kvp->key(), kvp->value() );
            }
           else
            { bvo->insert( qobject_cast<KeyValueArray *>(makeKvb()) );
            }
          on_clear_clicked();
          updateBuild();
        }
       else if ( selBB->isArray() )
        { ValueBaseArray *vba = qobject_cast<ValueBaseArray *>(selBB);
          if ( (vba->vKey() & RDT_TYPEMASK) != (key & RDT_TYPEMASK) ) { qWarning( "Make item key 0x%llx incompatible with array key 0x%llx", key, vba->vKey() ); return; }
          if ( !makeKvb()->isKeyValuePair() ) { qWarning( "Cannot insert an array directly into another array" ); return; }
          qint32 i = ui->index->value();
          if ( !vba->insertAt( ((KeyValuePair *)makeKvb())->value(), i ) )
            qWarning( "insertAt(%d) failed", i );
          on_clear_clicked();
          updateBuild();
        }
       else
        { qWarning( "Selected item type %x not suitable for insertion", selBB->type() ); return; }
    }
}

void  BlockTool::insertKeyName( Utf8String nKey )
{ if ( !dict.codesContainName( nKey ) )
    { qWarning( "key comboBox text returns unknown key: %s", nKey.data() );
      return;
    }
  ValueBase *vbp = nullptr;
  quint8 tp = dict.codeFromCodeName( nKey ) & RDT_OBTYPEMASK;
  if ( tp == RDT_NULL )
    vbp = new BlockValueNull(this);
   else
    { switch ( tp )
        { case RDT_OBJECT   : vbp = new BlockValueObject   ( this ); break;
          case RDT_INT64    : vbp = new BlockValueInt64    ( ui->     intEdit->text().toLongLong(), this ); break;
          case RDT_RICEYINT : vbp = new BlockValueRiceyInt ( ui->riceyIntEdit->text().toLongLong(), this ); break;
          case RDT_RCODE    : vbp = new BlockValueRiceyCode( dict.codeFromCodeName( ui->rcodeEdit->currentText().toUtf8() ), this ); break;
          case RDT_MPZ      : vbp = new BlockValueMPZ      ( ui->mpzEdit->text().toUtf8(), this ); break;
          case RDT_MPQ      : vbp = new BlockValueMPQ      ( ui->mpqEdit->text().toUtf8(), this ); break;
          case RDT_BYTEARRAY: vbp = new BlockValueByteArray( QByteArray::fromHex( ui->byteArrayEdit->text().toUtf8() ), this ); break;
          case RDT_STRING   : vbp = new BlockValueString   ( ui->stringEdit->text().toUtf8(), this ); break;
          case RDT_OBJECT_ARRAY   : vbp = new BlockValueObjectArray   ( this ); (qobject_cast<BlockValueObjectArray *   >(vbp))->append(new BlockValueObject   (vbp)); break;
          case RDT_INT64_ARRAY    : vbp = new BlockValueInt64Array    ( this ); (qobject_cast<BlockValueInt64Array *    >(vbp))->append(new BlockValueInt64    (ui->     intEdit->text().toLongLong(), vbp)); break;
          case RDT_RICEYINT_ARRAY : vbp = new BlockValueRiceyIntArray ( this ); (qobject_cast<BlockValueRiceyIntArray * >(vbp))->append(new BlockValueRiceyInt (ui->riceyIntEdit->text().toLongLong(), vbp)); break;
          case RDT_RCODE_ARRAY    : vbp = new BlockValueRiceyCodeArray( this ); (qobject_cast<BlockValueRiceyCodeArray *>(vbp))->append(new BlockValueRiceyCode(dict.codeFromCodeName(ui->rcodeEdit->currentText().toUtf8() ), vbp)); break;
          case RDT_MPZ_ARRAY      : vbp = new BlockValueMPZArray      ( this ); (qobject_cast<BlockValueMPZArray *      >(vbp))->append(new BlockValueMPZ      (ui->mpzEdit->text().toUtf8(), vbp)); break;
          case RDT_MPQ_ARRAY      : vbp = new BlockValueMPQArray      ( this ); (qobject_cast<BlockValueMPQArray *      >(vbp))->append(new BlockValueMPQ      (ui->mpqEdit->text().toUtf8(), vbp)); break;
          case RDT_BYTEARRAY_ARRAY: vbp = new BlockValueByteArrayArray( this ); (qobject_cast<BlockValueByteArrayArray *>(vbp))->append(new BlockValueByteArray(QByteArray::fromHex( ui->byteArrayEdit->text().toUtf8() ), vbp)); break;
          case RDT_STRING_ARRAY   : vbp = new BlockValueStringArray   ( this ); (qobject_cast<BlockValueStringArray *   >(vbp))->append(new BlockValueString   (ui->stringEdit->text().toUtf8(), vbp)); break;
        }
    }
  if ( vbp == nullptr )
    { qWarning( "no value or array established." );
      ui->report->append( "oops" );
      return;
    }
  ui->report->clear();
  ui->report->append( "set clicked" );
  KeyValueBase *kvb = makeKvb();
  if ( kvb != nullptr )
    { kvb->deleteLater();
      kvb = nullptr;
    }
  RiceyInt kc = dict.codeFromCodeName( nKey );
  if ( ( tp & RDT_ARRAY ) == 0 )
    kvb = new KeyValuePair( kc, vbp, this );
   else
    kvb = new KeyValueArray( kc, (ValueBaseArray *)vbp, this );
  setMake( kvb );
  makeKvb()->setValueKey( kc );
  if ( ui->showJson->isChecked() )ui->report->append( jsonReformat( kvb->json() ) );
  if ( ui->showBao ->isChecked() )ui->report->append( kvb->bao().toHex() );
  if ( ui->showDot ->isChecked() )ui->report->append( kvb->dot(ValueBase::Mode::idle) ); // TODO: get the current mode from BlockPanel
  // kvp->deleteLater(); child of vbp, no need to delete this
  vbp->deleteLater();
}


void  BlockTool::on_key_currentTextChanged( const QString &nuKey )
{ Utf8String nKey = nuKey.toUtf8();
  if ( !dict.codesContainName( nKey ) )
    { qWarning( "key comboBox selected unknown key: %s", nKey.data() );
      return;
    }
  quint8 tp = dict.codeFromCodeName( nKey ) & RDT_OBTYPEMASK;
  switch( tp )
    { case RDT_NULL           : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_null      )); break;
      case RDT_OBJECT_ARRAY   :
      case RDT_OBJECT         : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_object    )); break;
      case RDT_INT64_ARRAY    :
      case RDT_INT64          : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_int       )); break;
      case RDT_RICEYINT_ARRAY :
      case RDT_RICEYINT       : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_riceyInt  )); break;
      case RDT_RCODE_ARRAY    :
      case RDT_RCODE          : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_rcode     )); break;
      case RDT_MPZ_ARRAY      :
      case RDT_MPZ            : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_mpz       )); break;
      case RDT_MPQ_ARRAY      :
      case RDT_MPQ            : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_mpq       )); break;
      case RDT_BYTEARRAY_ARRAY:
      case RDT_BYTEARRAY      : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_byteArray )); break;
      case RDT_STRING_ARRAY   :
      case RDT_STRING         : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_string    )); break;
//      case X_ARRAY   : ui->valueWidget->setCurrentIndex( ui->valueWidget->indexOf(ui->page_array     )); break;
      default: qWarning( "key comboBox selected unhandled type %d key: %s", tp, nKey.data() );
    }
}


void  BlockTool::on_navMake_toggled( bool make )
{ if ( !make )
    return;
  if ( ui->makeX->isChecked() ) { if ( !swapping ) selectRoot( panelX ); panelX->update(); }
  if ( ui->makeY->isChecked() ) { if ( !swapping ) selectRoot( panelY ); panelY->update(); }
  updateBuild();
  updateValueEditor();
  ui->remove->setStyleSheet( buttonStyle( ValueBase::Mode::make ) );
  ui->insert->setStyleSheet( buttonStyle( ValueBase::Mode::make ) );
  ui->set   ->setStyleSheet( buttonStyle( ValueBase::Mode::make ) );
}

void  BlockTool::on_navBuild_toggled( bool build )
{ if ( !build )
    { if ( !swapping )
        { selBuildBao.clear();
          if ( buildPanel() )
            if ( buildPanel()->m_kvb )
              selBuildBao = buildPanel()->m_kvb->bao();
          if ( selBuildBao.size() < 1 )
            selBuild = nullptr;
           else
            selBuild = selBB;
        }
      return;
    }
  if ( !swapping )
    { if ( selBuild != nullptr )
        { if ( selBuildBao.size() > 0 )
            { if ( buildPanel() ) // TODO: better test of "is it the s
                { if ( buildPanel()->m_kvb->bao() == selBuildBao )
                    updateBB( selBuild );
                   else
                    selBuild = nullptr;
                 }
                else
                 selBuild = nullptr;
            }
           else
            selBuild = nullptr;
        }
    }
   else
    selBuild = nullptr;
  if ( ui->buildA->isChecked() ) { if ( !swapping ) { if ( selBuild == nullptr ) selectRoot( panelA ); } panelA->update(); }
  if ( ui->buildX->isChecked() ) { if ( !swapping ) { if ( selBuild == nullptr ) selectRoot( panelX ); } panelX->update(); }
  if ( ui->buildY->isChecked() ) { if ( !swapping ) { if ( selBuild == nullptr ) selectRoot( panelY ); } panelY->update(); }
  updateMake();
  updateValueEditor();
  ui->remove->setStyleSheet( buttonStyle( ValueBase::Mode::build ) );
  ui->insert->setStyleSheet( buttonStyle( ValueBase::Mode::build ) );
  ui->set   ->setStyleSheet( buttonStyle( ValueBase::Mode::build ) );
}

void  BlockTool::on_sortName_toggled(bool) { sortKeys(); }
void  BlockTool::on_sortId_toggled  (bool) { sortKeys(); }
void  BlockTool::on_sortDict_toggled(bool) { sortKeys(); }
void  BlockTool::sortKeys()
{ QSignalBlocker blockKey(ui->key);
  QSignalBlocker blockRcodeEdit(ui->rcodeEdit);
  ui->key->clear();
  ui->rcodeEdit->clear();

  if ( ui->sortName->isChecked() )
    { QList<Utf8String> keyNames = dict.names();
      std::sort(keyNames.begin(), keyNames.end());
      for ( qint32 i = 0; i < keyNames.size(); i++ )
        { ui->key      ->insertItem( i, keyNames.at(i) );
          ui->rcodeEdit->insertItem( i, keyNames.at(i) );
        }
    }
   else if ( ui->sortDict->isChecked() )
    { QList<qint32> ind = dict.diNames.keys();
      std::sort(ind.begin(), ind.end());
      for ( qint32 i = 0; i < ind.size(); i++ )
        { ui->key      ->insertItem( i, dict.diNames[ind.at(i)] );
          ui->rcodeEdit->insertItem( i, dict.diNames[ind.at(i)] );
        }
    }
   else if ( ui->sortId->isChecked() )
    { QList<RiceyInt> ri = dict.ciByNum.keys();
      std::sort(ri.begin(), ri.end());
      for ( qint32 i = 0; i < ri.size(); i++ )
        { ui->key      ->insertItem( i, dict.nameFromCode(ri.at(i)) );
          ui->rcodeEdit->insertItem( i, dict.nameFromCode(ri.at(i)) );
        }
    }
   else
    qWarning( "no key sort method checked" );
  on_key_currentTextChanged( ui->key->currentText() );
}

void  BlockTool::on_makeX_toggled(bool c)
{ if ( !c && selBB && ui->navMake->isChecked() )
    { selBB->clearSel();
   //   ui->navGroup->setEnabled( false );
    }
  if ( c && ui->buildX->isChecked() )
    ui->buildA->setChecked(true);
  if ( c && ui->navMake->isChecked() )
    selectRoot( panelX );
   else
    updateValueEditor();
  panelX->setMode( c ? ValueBase::Mode::make : ValueBase::Mode::idle );
}
void  BlockTool::on_makeY_toggled(bool c)
{ if ( !c && selBB && ui->navMake->isChecked() )
    { selBB->clearSel();
   // ui->navGroup->setEnabled( false );
    }
  if ( c && ui->buildY->isChecked() )
    ui->buildA->setChecked(true);
  if ( c && ui->navMake->isChecked() )
    selectRoot( panelY );
  panelY->setMode( c ? ValueBase::Mode::make : ValueBase::Mode::idle );
  // updateValueEditor(); will happen on the makeX_toggled side
}
void  BlockTool::on_buildA_toggled(bool c)
{ if ( !c && selBB && ui->navBuild->isChecked() )
    { selBB->clearSel();
     // ui->navGroup->setEnabled( false );
    }
  if ( c && ui->navBuild->isChecked() )
    selectRoot( panelA );
  panelA->setMode( c ? ValueBase::Mode::build : ValueBase::Mode::idle );
}
void  BlockTool::on_buildX_toggled(bool c)
{ if ( !c && selBB && ui->navBuild->isChecked() )
    { selBB->clearSel();
    //  ui->navGroup->setEnabled( false );
    }
  if ( c && ui->makeX->isChecked() )
    ui->makeY->setChecked(true);
  if ( c && ui->navBuild->isChecked() )
    selectRoot( panelX );
  panelX->setMode( c ? ValueBase::Mode::build : ValueBase::Mode::idle );
}
void  BlockTool::on_buildY_toggled(bool c)
{ if ( !c && selBB && ui->navBuild->isChecked() )
    { selBB->clearSel();
    //  ui->navGroup->setEnabled( false );
    }
  if ( c && ui->makeY->isChecked() )
    ui->makeX->setChecked(true);
  if ( c && ui->navBuild->isChecked() )
    selectRoot( panelY );
  panelY->setMode( c ? ValueBase::Mode::build : ValueBase::Mode::idle );
}

void  BlockTool::selectRoot(BlockPanel *bp)
{ if ( bp == nullptr )
    return;
  if ( bp->m_kvb == nullptr )
    { updateBB( nullptr );
      return;
    }
  KeyValueBase *p_kvb = bp->m_kvb;
  ValueBase *vb = nullptr;
  if (( p_kvb->key() & RDT_ARRAY ) == 0 )
    vb = ( (KeyValuePair *)p_kvb)->value();
   else
    vb = ((KeyValueArray *)p_kvb)->value();
//   ui->navGroup->setEnabled( vb != nullptr );
   updateBB( vb );
}

void  BlockTool::on_swap_clicked()
{ BlockPanel *mp = makePanel();
  BlockPanel *bp = buildPanel();
  if (( mp == nullptr ) || ( bp == nullptr ))
    { qWarning( "swap - cannot swap, panel undefined" );
      return;
    }
  swapping = true;
  KeyValueBase *vb = mp->m_kvb;
  mp->m_kvb = bp->m_kvb;
  bp->m_kvb = vb;
  // Setting the opposite nav mode true (instead of setting the current one false)
  //  is important to the order of operations that results during updates and redraws.
  if ( ui->navBuild->isChecked() )
    ui->navMake ->setChecked( true );
   else
    ui->navBuild->setChecked( true );
  swapping = false;
}

void  BlockTool::on_prev_clicked()
{ if ( selBB == nullptr )
    { if ( ui->navBuild->isChecked() )
        ui->navMake->setChecked( true );
       else
        ui->navBuild->setChecked( true );
      return;
    }
  ValueBase *vb = nullptr;
  ValueBase *vbp = selBB->vbParent;
  if ( vbp == nullptr )
    { qWarning( "TODO: handle the case where the top level container is a KeyValueArray..." );
    }
   else
    { if ( vbp->isContainer() )
        vb = vbp->prevChild( selBB );
       else
        qWarning( "when is a parent not a container?" );
    }

  if ( vb )
    { ui->navGroup->setEnabled( false );
      navPanel = curNavPanel();
      if ( ui->showMeta->isChecked() )
        showMeta( vb );
      updateBB( vb );
      if ( navPanel )
        { connect( navPanel, SIGNAL(drawingComplete()), this, SLOT(navDrawComplete()) );
          navPanel->update();
        }
       else
        qWarning( "neither nav button checked." );
    }
}

void  BlockTool::navDrawComplete()
{ disconnect( navPanel, SIGNAL(drawingComplete()), this, SLOT(navDrawComplete()) );
  ui->navGroup->setEnabled( true );
}

BlockPanel *BlockTool::curNavPanel()
{ if ( ui->navBuild->isChecked() ) return buildPanel();
  if ( ui->navMake ->isChecked() ) return makePanel();
  return nullptr;
}

/**
 * @brief BlockTool::on_next_clicked - depth first search
 */
void  BlockTool::on_next_clicked()
{ if ( selBB == nullptr )
    { if ( ui->navBuild->isChecked() )
        ui->navMake->setChecked( true );
       else
        ui->navBuild->setChecked( true );
      return;
    }
  ValueBase *vb = nullptr;
  if ( selBB->isContainer() )
    if ( selBB->size() > 0 )
      vb = selBB->firstChild();

  if ( vb == nullptr )
   if ( selBB->vbParent )
     vb = selBB->vbParent->nextChild( selBB );

  if ( vb )
    { ui->navGroup->setEnabled( false );
      navPanel = curNavPanel();
      updateBB( vb );
      if ( ui->showMeta->isChecked() )
        showMeta( vb );
      if ( navPanel )
        { connect( navPanel, SIGNAL(drawingComplete()), this, SLOT(navDrawComplete()) );
          navPanel->update();
        }
       else
        qWarning( "neither nav button checked." );
    }
}

void  BlockTool::updateBB( ValueBase *vb )
{ if ( selBB )
    selBB->clearSel();
  selBB = vb;
  if ( selBB )
    { selBB->setSel();
      updateValueEditor();
    }
}

void  BlockTool::showMeta( ValueBase *vb )
{ if ( vb == nullptr )
    { qWarning( "showMeta received nullptr" );
      return;
    }
  ui->report->append( vb->metaText() );
}

void  BlockTool::on_now_clicked()
{ // TODO: parse and add time in the plus line edit
  qint64 t = QDateTime::currentMSecsSinceEpoch()*1000;
  ui->intEdit->setText( QString::number(t) );
}

BlockPanel *BlockTool::makePanel()
{ if ( ui->makeX->isChecked() ) return panelX;
  if ( ui->makeY->isChecked() ) return panelY;
  qWarning( "BlockTool::makePanel() no make panel checked" );
  return nullptr;
}

BlockPanel *BlockTool::buildPanel()
{ if ( ui->buildA->isChecked() ) return panelA;
  if ( ui->buildX->isChecked() ) return panelX;
  if ( ui->buildY->isChecked() ) return panelY;
  qWarning( "BlockTool::buildPanel() no build panel checked" );
  return nullptr;
}

BlockPanel *BlockTool::updateMake()
{ if ( ui->makeX->isChecked() ) { panelX->update(); return panelX; }
  if ( ui->makeY->isChecked() ) { panelY->update(); return panelY; }
  qWarning( "BlockTool::updateMake() no make panel checked" );
  return nullptr;
}

BlockPanel *BlockTool::updateBuild()
{ if ( ui->buildA->isChecked() ) { panelA->update(); return panelA; }
  if ( ui->buildX->isChecked() ) { panelX->update(); return panelX; }
  if ( ui->buildY->isChecked() ) { panelY->update(); return panelY; }
  qWarning( "BlockTool::updateBuild() no build panel checked" );
  return nullptr;
}

BlockPanel *BlockTool::updateNav()
{ if ( ui->navBuild->isChecked() ) return updateBuild();
  if ( ui->navMake ->isChecked() ) return updateMake();
  qWarning( "BlockTool::updateNav() no nav choice checked" );
  return nullptr;
}


bool  BlockTool::setBuild( KeyValueBase *kvb )
{ BlockPanel *bp = buildPanel();
  if ( bp )
    { bp->setKeyValueBlock( kvb, false );
      if ( ui->navBuild->isChecked() )
        selectRoot( bp );
      bp->update();
      return true;
    }
  qWarning( "BlockTool::setBuild() no build panel checked" );
  return false;
}

bool  BlockTool::setMake( KeyValueBase *kvb )
{ BlockPanel *bp = makePanel();
  if ( bp )
    { bp->setKeyValueBlock( kvb, false );
      if ( ui->navMake->isChecked() )
        selectRoot( bp );
      bp->update();
      return true;
    }
  qWarning( "BlockTool::setMake() no make panel checked" );
  return false;
}

QString  BlockTool::buildLabel()
{ if ( ui->buildA->isChecked() ) { return panelA->label(); }
  if ( ui->buildX->isChecked() ) { return panelX->label(); }
  if ( ui->buildY->isChecked() ) { return panelY->label(); }
  qWarning( "BlockTool::buildLabel() no build panel checked" );
  return "-";
}

QString  BlockTool::makeLabel()
{ if ( ui->makeX->isChecked() ) { return panelX->label(); }
  if ( ui->makeY->isChecked() ) { return panelY->label(); }
  qWarning( "BlockTool::makeLabel() no make panel checked" );
  return "-";
}

KeyValueBase *BlockTool::buildKvb()
{ if ( ui->buildA->isChecked() ) { return panelA->kvb(); }
  if ( ui->buildX->isChecked() ) { return panelX->kvb(); }
  if ( ui->buildY->isChecked() ) { return panelY->kvb(); }
  qWarning( "BlockTool::buildKvb() no build panel checked" );
  return nullptr;
}

KeyValueBase *BlockTool::makeKvb()
{ if ( ui->makeX->isChecked() ) { return panelX->kvb(); }
  if ( ui->makeY->isChecked() ) { return panelY->kvb(); }
  qWarning( "BlockTool::makeKvb() no make panel checked" );
  return nullptr;
}

KeyValueBase *BlockTool::curNavKvb()
{ if ( ui->navBuild->isChecked() ) return buildKvb();
  if ( ui->navMake ->isChecked() ) return makeKvb();
  qWarning( "BlockTool::curNavKvb() no nav mode checked" );
  return nullptr;
}

/**
 * @brief BlockTool::makeClear - clear the make window
 * @return true if successful
 */
bool  BlockTool::makeClear()
{ BlockPanel *bp = makePanel();
  if ( bp )
    { bp->clear();
      if ( ui->navMake->isChecked() )
        updateBB( nullptr );
      updateValueEditor();
      return true;
    }
  qWarning( "BlockTool::makeClear() no make panel checked" );
  return false;
}

/**
 * @brief BlockTool::on_read_clicked - read either from any file or the selected
 *   resource .bao file into the Make window
 */
void  BlockTool::on_read_clicked()
{ QString readFile;
  if ( ui->readFile->currentText() == "Open File" )
    readFile = QFileDialog::getOpenFileName(this,QString("Read Into %1").arg( makeLabel() ), fileDir );
   else
    readFile = ":/files/"+ui->readFile->currentText()+".bao";
  if ( readFile.size() < 1 )
    return;
  QFile file(readFile);
  if ( !file.open(QIODevice::ReadOnly) )
    { qWarning( "could not open %s for reading", readFile.toUtf8().data() );
      return;
    }
   QFileInfo fi = QFileInfo( file );
  Utf8String fn = fi.fileName().toUtf8();
        QDir di = fi.dir();
  fileDir = di.absolutePath();
  if ( readFile.endsWith( ".json", Qt::CaseInsensitive ) )
    { JsonSerial js = file.readAll();
      if ( js.size() > 4 )
        { KeyValueBase *kvb = makeKvb();
          if ( kvb )
            on_clear_clicked();
          kvb = KeyValueBase::readJson( js, this );
          setMake( kvb );
        }
       else
        qWarning( "valid json not found in %s", fn.data() );
      return;
    }
  BaoSerial bs = file.readAll();
  if ( bs.size() > 0 )
    { KeyValueBase *kvb = makeKvb();
      if ( kvb )
        on_clear_clicked();
      kvb = KeyValueBase::readBao( bs, this );
      setMake( kvb );
    }
   else
    qWarning( "read no data from %s", fn.data() );
}

/**
 * @brief BlockTool::on_save_clicked - write a serialized representation
 *   of the Build window to a file.
 */
void  BlockTool::on_save_clicked()
{ KeyValueBase *kvb = buildKvb();
  if ( kvb == nullptr )
    return;
  QString saveFile = QFileDialog::getSaveFileName(this,QString("Save %1").arg( buildLabel() ), fileDir );
  if ( saveFile.size() < 1 )
    return;
  QFile file(saveFile);
  if ( !file.open(QIODevice::WriteOnly) )
    { qWarning( "could not open %s for writing", saveFile.toUtf8().data() );
      return;
    }
  QFileInfo fi = QFileInfo( file );
  QDir di = fi.dir();
  fileDir = di.absolutePath();
  if ( saveFile.endsWith( ".json", Qt::CaseInsensitive ) )
    { QTextStream ts( &file );
      ts << jsonReformat( kvb->json() );
      return;
    }
  file.write( kvb->bao() );
}

/**
 * @brief BlockTool::liveDelay - allow UI to update while delaying this thread's execution
 * @param t - milliseconds to delay
 */
void BlockTool::liveDelay( int t )
{ QTimer timer;
  timer.setSingleShot( true );
  timer.start( t );
  QCoreApplication::processEvents( QEventLoop::AllEvents, t );
  while ( timer.isActive() )
    { usleep( 5000 ); // Get 5ms of "real sleep"
      QCoreApplication::processEvents( QEventLoop::AllEvents, 50 );
    }
}

/**
 * @brief BlockTool::jsonReformat
 * @param j - json stream to reformat
 * @return j - prettified by the QJsonDocument default output format (alphabetized keys, indented)
 */
QByteArray  BlockTool::jsonReformat( QByteArray j )
{ QJsonDocument jd = QJsonDocument::fromJson( j );
  return jd.toJson();
}

/**
 * @brief BlockTool::curNavMode
 * @return mode that the Navigation tool is active on
 */
ValueBase::Mode BlockTool::curNavMode()
{                  ValueBase::Mode mode = ValueBase::Mode::idle ;
  if ( ui->navBuild->isChecked() ) mode = ValueBase::Mode::build;
  if ( ui->navMake ->isChecked() ) mode = ValueBase::Mode::make ;
  return mode;
}

/**
 * @brief BlockTool::on_show_clicked - show the selected text representations of
 *   the currently navigating keyValue object.
 */
void  BlockTool::on_show_clicked()
{ ui->report->clear();
  KeyValueBase *kvb = curNavKvb();
  if ( kvb == nullptr )
    return;
  if ( ui->showJson->isChecked() ) ui->report->append( jsonReformat( kvb->json() ) );
  if ( ui->showBao ->isChecked() ) ui->report->append( QString::fromUtf8( kvb->bao().toHex() ) );
  if ( ui->showDot ->isChecked() ) ui->report->append( kvb->dot(curNavMode()) );
}

/**
 * @brief BlockTool::initReadFile - readFile is a comboBox which either specifies
 *   a resource file, or to open a file selection dialog.
 */
void  BlockTool::initReadFile()
{ ui->readFile->clear();
  ui->readFile->addItem( "Open File" );
  QDir d(":/files");
  QFileInfoList fil = d.entryInfoList();
  foreach( QFileInfo fi, fil )
    { QString fn = fi.fileName();
      if ( fn.endsWith( ".bao" ) )
        ui->readFile->addItem( fn.mid( 0, fn.size()-4) );
    }
}

/**
 * @brief BlockTool::buttonStyle - stylesheet for buttons that change with mode
 * @param m - controls the button outline color
 * @return style sheet for the color outline buttons, color chosen by mode
 */
QString BlockTool::buttonStyle( ValueBase::Mode m )
{ QString color;
  switch ( m )
    { case ValueBase::Mode::make:  color = "green"; break;
      case ValueBase::Mode::build: color = "blue";  break;
      default:                     color = "black";
    }
  QString s =
  QString( "QPushButton {\
    padding: 2px;\
    border: 2px solid %1;\
    border-radius: 5px;\
    min-width: 50px;\
    background-color: ghostwhite;\
  }\
QPushButton:pressed {\
    background-color: qlineargradient(x1: 0, y1: 0, x2: 0, y2: 1,\
                        stop: 0 #dadbde, stop: 1 #f6f7fa);\
  }").arg( color );
  return s;
}
