#!/bin/bash
# Removes the HP LaserJet 1020 native driver. Double-click to run (asks for your password).
echo "Removing the HP LaserJet 1020 native driver..."
sudo /bin/bash -c '
    for q in $(lpstat -v 2>/dev/null | awk -F"[ :]+" "/hp1020:/{print \$3}"); do lpadmin -x "$q"; done
    rm -f /usr/libexec/cups/filter/rastertozjs /usr/libexec/cups/backend/hp1020
    rm -f "/Library/Printers/PPDs/Contents/Resources/HP LaserJet 1020 Native.ppd"
    rm -rf /Library/Printers/hp1020
    pkgutil --forget io.github.utkarshchandel.hp1020-driver >/dev/null 2>&1
' && echo "Done. You can close this window."
