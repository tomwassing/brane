#!/bin/bash
# OPEN PORTS.sh
#   by Lut99
#
# Created:
#   09 Feb 2022, 16:19:46
# Last edited:
#   09 Feb 2022, 16:23:40
# Auto updated?
#   Yes
#
# Description:
#   Script to open ports for a simple Xenon-SSH remote case.
#   Does so by adding firewall rules for this session only.
#

# Try to switch to root
echo "Checking for root permission..."
sudo echo " > Running as root"
echo ""

# Add the rule for the registry
echo "Setting rule for Brane registry..."
sudo iptables -A services -p tcp -m state --state NEW -m tcp --dport 5000 -j ACCEPT -m comment --comment "Brane registry"

# Add the rules for JuiceFS
echo "Setting rules for JuiceFS (minio & redis) registry..."
sudo iptables -A services -p tcp -m state --state NEW -m tcp --dport 9000 -j ACCEPT -m comment --comment "Brane minio (JuiceFS)"
sudo iptables -A services -p tcp -m state --state NEW -m tcp --dport 6379 -j ACCEPT -m comment --comment "Brane redis (JuiceFS)"

# Add the rule for the callback
echo "Setting rule for Brane callback..."
sudo iptables -A services -p tcp -m state --state NEW -m tcp --dport 50052 -j ACCEPT -m comment --comment "Brane callback"

echo ""
echo "Done."
echo ""
