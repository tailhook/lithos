#!/bin/sh -e

sudo -k
echo Copying examples/py into the system
echo WARNING: this will remove /etc/lithos from the system
echo ... hoping you run this in virtual machine
echo ... but let you think 10 seconds

for i in $(seq 10 -1 0); do echo -n "$i \r"; sleep 1; done;
echo Okay proceeding...

sudo rsync -av --delete-after examples/py/configs/ /etc/lithos

vagga _build py-example

[ -d /var/lib/lithos/images ] || sudo mkdir -p /var/lib/lithos/images

sudo rsync -a --delete-after .vagga/py-example/ /var/lib/lithos/images/py-example

echo Done.
echo Ensure that you have run '`vagga run`' before.
echo Then run '`sudo ./target/debug/lithos_tree`' or whatever command you wish

