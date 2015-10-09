#!/bin/sh -e

sudo -k
echo Copying examples/sleep into the system
echo WARNING: this will remove /etc/lithos from the system
echo ... hoping you run this in virtual machine
echo ... but let you think 10 seconds

for i in $(seq 10 -1 0); do echo -n "$i \r"; sleep 1; done;
echo Okay proceeding...

sudo rsync -av --delete-after examples/sleep/ /etc/lithos

vagga _build busybox

[ -d /var/lib/lithos/images ] || sudo mkdir -p /var/lib/lithos/images

sudo rsync -a --delete-after .vagga/busybox/ /var/lib/lithos/images/busybox

echo Done.
echo Ensure that you have run '`vagga run`' before.
echo Then run '`sudo ./target/debug/lithos_tree`' or whatever command you wish

