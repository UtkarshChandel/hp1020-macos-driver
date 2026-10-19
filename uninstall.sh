#!/usr/bin/env bash
# Remove the queue, filter, backend, PPD and firmware installed by install.sh.
set -uo pipefail
lpadmin -x HP_LaserJet_1020 2>/dev/null || true
sudo rm -f /usr/libexec/cups/filter/rastertozjs /usr/libexec/cups/backend/hp1020
sudo rm -rf /Library/Printers/hp1020
echo "Removed."
