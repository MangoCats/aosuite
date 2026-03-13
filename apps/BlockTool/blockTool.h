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
#ifndef BLOCKTOOL_H
#define BLOCKTOOL_H

#include <QScrollArea>
#include <QProcess>
#include "blockOb.h"
#include "blockPanel.h"

namespace Ui {
class BlockTool;
}

class BlockTool : public QScrollArea
{
    Q_OBJECT

public:
    explicit  BlockTool( QWidget *cw = nullptr );
             ~BlockTool();
        void  closing();
        void  restore();
        void  liveDelay( int t );
        void  initReadFile();
  QByteArray  jsonReformat( QByteArray );
        void  updateBB( ValueBase * );
  BlockPanel *buildPanel();
  BlockPanel *makePanel();
  BlockPanel *curNavPanel();
  BlockPanel *updateBuild();
  BlockPanel *updateMake();
  BlockPanel *updateNav();
        bool  setBuild( KeyValueBase * );
        bool  setMake( KeyValueBase * );
     QString  buildLabel();
     QString  makeLabel();
KeyValueBase *buildKvb();
KeyValueBase *makeKvb();
KeyValueBase *curNavKvb();
ValueBase::Mode curNavMode();
        void  showMeta( ValueBase * );
        bool  makeClear();
        void  sortKeys();
        void  selectRoot(BlockPanel *);
        void  updateValueEditor();
        void  valueEditorNoNav();
        void  selectObjectForInsertion();
        void  editSelectedElement();
        void  setEditorValue( ValueBase * );
        void  insertKeyName( Utf8String );
    RiceyInt  removeArrayElement( ValueBase * );
    RiceyInt  removeObjectElement( ValueBase * );
     QString  buttonStyle( ValueBase::Mode m );

signals:
        void  showA( KeyValueBase * );
        void  showX( KeyValueBase * );
        void  showY( KeyValueBase * );

public slots:
        void  on_remove_clicked();
        void  on_insert_clicked();
        void  on_show_clicked();
        void  on_swap_clicked();
        void  on_prev_clicked();
        void  on_next_clicked();
        void  on_now_clicked();
        void  on_set_clicked();
        void  on_key_currentTextChanged(const QString &);
        void  on_read_clicked();
        void  on_save_clicked();
        void  on_clear_clicked() { makeClear(); }
        void  on_navMake_toggled(bool);
        void  on_navBuild_toggled(bool);
        void  on_makeX_toggled(bool);
        void  on_makeY_toggled(bool);
        void  on_buildA_toggled(bool);
        void  on_buildX_toggled(bool);
        void  on_buildY_toggled(bool);
        void  on_sortName_toggled(bool);
        void  on_sortId_toggled(bool);
        void  on_sortDict_toggled(bool);
        void  navDrawComplete();

public:
   QPointer<BlockPanel> panelA;
   QPointer<BlockPanel> panelX;
   QPointer<BlockPanel> panelY;
               QString  fileDir;
         Ui::BlockTool *ui;
    QPointer<ValueBase> selBB;
    QPointer<ValueBase> selBuild;
             BaoSerial  selBuildBao;
     QPointer<QProcess> pp;
            BlockPanel *navPanel;
                  bool  swapping;
};

#endif // BLOCKTOOL_H
