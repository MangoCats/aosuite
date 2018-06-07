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
#include "keypair.h"

/**
 * @brief KeyPair::KeyPair - constructor
 * @param di - optional data item
 * @param p - optional parent object
 */
KeyPair::KeyPair( DataItemBA di, QObject *p )
  : DataVarLength( AO_KEYPAIR, p )
{ // See if there's anything interesting in the data item
  if ( di.size() > 0 )
    { if ( typeCodeOf( di ) != AO_KEYPAIR )
        { // TODO: log an error
          return;
        }
       else
        { DataVarLength temp( di );        // It's our type
          DataItemBA items = temp.get();  // typeCode has been stripped off
          while ( items.size() > 0 )
            { int sz = typeSize( items );
              if ( sz <= 0 )
                { // TODO: log error
                  return;
                }
               else
                { switch ( typeCodeOf( items ) ) // read valid items from the byte array, in any order
                    { case AO_ECDSA_PUB_KEY2:
                      case AO_ECDSA_PUB_KEY3:
                      case AO_RSA3072_PUB_KEY:
                      // case AO_ID_SEQ_NUM: // TODO: this one needs a database lookup
                        pubKey = items;
                        break;

                      case AO_ECDSA_PRI_KEY:
                      case AO_RSA3072_PRI_KEY:
                        priKey = items;
                        break;

                      default:
                        // TODO: log anomaly - unrecognized data type
                        break;
                    }
                  items = items.mid( sz ); // move on to the next
                }
            }
        }
    }
}

/**
 * @brief KeyPair::operator =
 * @param di - data item to assign
 */
void KeyPair::operator = ( const DataItemBA &di )
{ KeyPair temp( di );
  pubKey   = temp.pubKey;
  priKey   = temp.priKey;
  typeCode = temp.typeCode;
  return;
}

/**
 * @brief KeyPair::toDataItem
 * @param cf - compact (or chain) form?  Pass along to children.
 * @return data item with the BlockRef contents
 */
DataItemBA  KeyPair::toDataItem( bool cf )
{ QByteArrayList dil;
  if ( pubKey.isValid() )
    dil.append( pubKey.toDataItem(cf) );
  if ( priKey.isValid() )
    dil.append( priKey.toDataItem(cf) );
  std::sort( dil.begin(), dil.end() );
  ba = dil.join();
  return DataVarLength::toDataItem(cf);
}

#define SHOW_IF_GPGERR( error ) \
if ( error )                     \
  { qDebug( "Show %s: %s (%d)",   \
     gpgme_strsource(error),       \
     gpgme_strerror (error),        \
                     error );        \
  }

#define FAIL_IF_GPGERR( error ) \
if ( error )                     \
  { qDebug( "Fail %s: %s (%d)",   \
     gpgme_strsource(error),       \
     gpgme_strerror (error),        \
                     error );        \
    return false;                     \
  }


gpgme_error_t KeyPair::initGpgme()
{ if ( !gpgme_check_version( "1.8.1" ) )
    { qDebug( "libpgme version fails to meet 1.8.1 expectation" ); }
  setlocale( LC_ALL, "" );
  gpgme_set_locale( NULL, LC_CTYPE   , setlocale( LC_CTYPE   , NULL ) );
  gpgme_set_locale( NULL, LC_MESSAGES, setlocale( LC_MESSAGES, NULL ) );
  return gpgme_engine_check_version( GPGME_PROTOCOL_OpenPGP );
}

/**
 * @brief KeyPair::makeNewPair
 * @param tc - type of keyPair to make (using the private key type).
 * @return true if successful;
 */
bool KeyPair::makeNewPair( typeCode_t tc )
{ qDebug( "KeyPair::makeNewPair( 0x%x )", tc );
  if (( tc != AO_ECDSA_PRI_KEY ) &&
      ( tc != AO_RSA3072_PRI_KEY ))
    { qDebug( "KeyPair::makeNewPair unrecognized type (0x%x)", tc );
      return false;
    }
  gpgme_ctx_t mContext;
  gpgme_engine_info_t info;
  gpgme_error_t  error = GPG_ERR_NO_ERROR;
  const char * CONFIG_DIR = "~/.gnupg/";

  // Initializes gpgme
  gpgme_check_version(NULL);

  // Initialize the locale environment.
  setlocale(LC_ALL, "");
  gpgme_set_locale(NULL, LC_CTYPE, setlocale(LC_CTYPE, NULL));
  #ifdef LC_MESSAGES
    gpgme_set_locale(NULL, LC_MESSAGES, setlocale(LC_MESSAGES, NULL));
  #endif
  FAIL_IF_GPGERR( gpgme_set_engine_info(GPGME_PROTOCOL_OpenPGP, NULL, CONFIG_DIR) );
  FAIL_IF_GPGERR( gpgme_new(&mContext) );
  FAIL_IF_GPGERR( gpgme_engine_check_version(GPGME_PROTOCOL_OpenPGP) ); // Check OpenPGP
  FAIL_IF_GPGERR( gpgme_get_engine_info(&info) );                      // load engine info
  while ( info && info->protocol != gpgme_get_protocol(mContext) )
    info = info->next;
  FAIL_IF_GPGERR( gpgme_ctx_set_engine_info(mContext, GPGME_PROTOCOL_OpenPGP, NULL, CONFIG_DIR) );  // set path to our config file

  QString def = QString( "<GnupgKeyParms format=\"internal\"> \n"
                    "key-type:      ECDSA\n"
                    "key-curve:     brainpoolP256r1\n"
                    "key-usage:     sign\n"
                    "subkey-type:   ECDH\n"
                    "subkey-curve:  brainpoolP256r1\n"
                    "subkey-usage:  encrypt\n"
                    "name-real:     Ⓐ\n"
//                    "name-email:    test@mail.com\n"
//                    "name-comment:  Ⓐ\n"
                    " Expire-Date: 2000 \n"
                    " Passphrase: abc \n"
                    " </GnupgKeyParms> \n" );

  FAIL_IF_GPGERR( gpgme_op_genkey(mContext, def.toStdString().c_str(), NULL, NULL ) );
  if (error == gpgme_err_code(GPG_ERR_NO_ERROR) )
    { gpgme_genkey_result_t res = gpgme_op_genkey_result(mContext);
      qDebug( "KeyPair::makeNewPair(0x%x) primary:%d sub:%d uid:%d fpr:%s"
              , tc, res->primary, res->sub, res->uid, res->fpr );

      if (res->primary && res->sub)
        return true;
    }
  qDebug( "fallthrough" );
  return false;
}


/**
 * @brief KeyPair::getGpgKeys - various ways of looking at the keys
 * @return false if failed somewhere
 */
bool KeyPair::getGpgKeys()
{ gpgme_ctx_t   ctx;
  gpgme_error_t err;
  gpgme_data_t  keydata;

  FAIL_IF_GPGERR( initGpgme() )
  FAIL_IF_GPGERR( gpgme_new( &ctx ) )
  FAIL_IF_GPGERR( gpgme_set_protocol( ctx, GPGME_PROTOCOL_OpenPGP ) )
  FAIL_IF_GPGERR( gpgme_set_protocol( ctx, GPGME_PROTOCOL_OpenPGP ) )
  gpgme_engine_info_t ei = gpgme_ctx_get_engine_info( ctx );
  qDebug( "Engine info:" );
  qDebug( " Protocol: %s", gpgme_get_protocol_name( ei->protocol ) );
  qDebug( " Filename: %s", ei->file_name );
  qDebug( " Home Dir: %s", ei->home_dir );
  qDebug( " Version : %s", ei->version );
  qDebug( " Req.Ver.: %s", ei->req_version ? ei->req_version : "NULL" );
  qDebug( gpgme_get_armor   (ctx) ? "ASCII Armored" : "NOT Armored" );
  qDebug( gpgme_get_textmode(ctx) ? "Canonical text mode" : "NOT text mode" );
  qDebug( gpgme_get_offline (ctx) ? "Offline mode enabled" : "Offline mode not enabled" );
  qDebug( "Keylist mode 0x%x", gpgme_get_keylist_mode(ctx) );

  FAIL_IF_GPGERR( gpgme_data_new(&keydata) )
  FAIL_IF_GPGERR( gpgme_op_keylist_start( ctx, NULL, 0 ) )

  int maxLoops = 3;
  int n = 0;
  bool done = false;
  while ( !done )
    { qDebug( "Getting key %d", n );
      err = gpgme_op_keylist_next( ctx, &(gpgKeys[n]) );
      if ( gpg_err_code(err) == GPG_ERR_EOF )
        done = true;
       else
        { FAIL_IF_GPGERR(err)
          gpgme_key_t key = gpgKeys[n];
          qDebug( "%s", qPrintable( QString( " got key %1 id:%2 name:%3 email:%4" )
                       .arg( n, 4 ).arg( key->subkeys->keyid )
                       .arg( (key->uids && key->uids->name ) ? key->uids->name  : "undefined" )
                       .arg( (key->uids && key->uids->email) ? key->uids->email : "undefined" ) ) );
          gpgme_subkey_t subkey = key->subkeys;
          err = 0;
          while ( subkey && !err )
            { qDebug( "Examining subkey:%s", subkey->keyid );
              subkey = subkey->next;
            }

          if ( ++n >= MAX_KEYS )
            { qDebug( "ran out of GPG keys storage, coded max of %d, looping.", MAX_KEYS );
              n = 0;
              if ( maxLoops-- <= 0 )
                { qDebug( "too many keys in GPG keyring, stopping." );
                  FAIL_IF_GPGERR( gpgme_op_keylist_end( ctx ) )
                  done = true;
                }
            }
        }
    }
  qDebug( "%d keys found", n );

  qDebug( "Looking for key Ⓐ" );
  SHOW_IF_GPGERR( gpgme_op_export( ctx, "Ⓐ", GPGME_EXPORT_MODE_MINIMAL, keydata ) )
  gpgme_data_seek( keydata, 0, SEEK_SET );
  gpgme_data_type_t dt = gpgme_data_identify(keydata,0);
  qDebug( "keydata type %d %s", dt, dt == GPGME_DATA_TYPE_PGP_KEY ? "That's a PGP KEY" : "unexpected type" );
  char buf[8192];
  ssize_t sz = gpgme_data_read( keydata, buf, 8192 );
  qDebug( "Read %ld bytes", sz );
  QByteArray ba( buf, sz );
  qDebug( "%s", qPrintable( QString::fromUtf8( ba.toHex() ) ) );

  gpgme_data_seek( keydata, 0, SEEK_SET );
  SHOW_IF_GPGERR( gpgme_op_export_keys( ctx, gpgKeys, GPGME_EXPORT_MODE_MINIMAL, keydata ) )
  gpgme_data_seek( keydata, 0, SEEK_SET );
  dt = gpgme_data_identify(keydata,0);
  qDebug( "keydata type %d %s", dt, dt == GPGME_DATA_TYPE_PGP_KEY ? "That's a PGP KEY" : "unexpected type" );
  sz = gpgme_data_read( keydata, buf, 8192 );
  qDebug( "Read %ld bytes", sz );
//  QByteArray ba2( buf, sz );
//  qDebug( "%s", qPrintable( QString::fromUtf8( ba2.toHex() ) ) );

  n = 0;
  done = false;
  while ( !done )
    { gpgme_key_t key = gpgKeys[n];
      if ( !key )
        done = true;
       else
        { qDebug( "%s", qPrintable( QString( " got key %1 id: %2 name:%3 email:%4" )
               .arg( n, 4 ).arg( key->subkeys->keyid )
               .arg( (key->uids && key->uids->name ) ? key->uids->name  : "undefined" )
               .arg( (key->uids && key->uids->email) ? key->uids->email : "undefined" ) ) );
          gpgme_subkey_t subkey = key->subkeys;
          err = 0;
          while ( subkey && !err )
            { qDebug( "Examining subkey:" );
              qDebug( "           keyid: %s", subkey->keyid            );
              qDebug( "Can authenticate: %d", subkey->can_authenticate );
              qDebug( "     Can certify: %d", subkey->can_certify      );
              qDebug( "     Can encrypt: %d", subkey->can_encrypt      );
              qDebug( "        Can sign: %d", subkey->can_sign         );
              qDebug( "     Card number: %s", subkey->card_number      );
              qDebug( "           Curve: %s", subkey->curve            );
              qDebug( "        Disabled: %d", subkey->disabled         );
              qDebug( "         Expired: %d", subkey->expired          );
              qDebug( "         Expires: %ld",subkey->expires          );
              qDebug( "             fpr: %s", subkey->fpr              );
              qDebug( "         invalid: %d", subkey->invalid          );
              qDebug( "     is card key: %d", subkey->is_cardkey       );
              qDebug( "        is de vs: %d", subkey->is_de_vs         );
              qDebug( "    is qualified: %d", subkey->is_qualified     );
              qDebug( "         keygrip: %s", subkey->keygrip          );
              qDebug( "          length: %d", subkey->length           );
              qDebug( "         revoked: %d", subkey->revoked          );
              qDebug( "          secret: %d", subkey->secret           );
              qDebug( "       timestamp: %ld",subkey->timestamp        );
              subkey = subkey->next;
            }
          n++;
        }
    }

  gpgme_data_release( keydata );
  gpgme_release( ctx );
  return true;
}
