#!/usr/bin/env bash
# Remove the driver (same as double-clicking /Library/Printers/hp1020/uninstall.command).
exec /bin/bash "$(dirname "${BASH_SOURCE[0]}")/packaging/uninstall.command"
