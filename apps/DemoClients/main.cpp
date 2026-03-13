#include "mainwindow.h"

#include <QApplication>
#include <QLocale>
#include <QTranslator>
#include <QSettings>

/**
 * @brief setAppNames - set names for QSettings use
 */
void setAppNames()
{ QCoreApplication::setOrganizationName( "AssignOnward" );
  QCoreApplication::setOrganizationDomain( "assignonward.com" );
  QCoreApplication::setApplicationName( "DemoClients" );
  QSettings::setDefaultFormat( QSettings::IniFormat );
}

int main(int argc, char *argv[])
{   setAppNames();
    QApplication a(argc, argv);

    QTranslator translator;
    const QStringList uiLanguages = QLocale::system().uiLanguages();
    for (const QString &locale : uiLanguages) {
        const QString baseName = "DemoClients_" + QLocale(locale).name();
        if (translator.load(":/i18n/" + baseName)) {
            a.installTranslator(&translator);
            break;
        }
    }
    MainWindow w;
    w.show();
    return a.exec();
}
