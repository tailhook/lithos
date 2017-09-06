#!/bin/sh -e

CONFIGS="${1:-py}"

sudo -k
echo Copying examples/py into the system
echo WARNING: This Command will remove /etc/lithos from the system
echo ... hopefully you run this in a virtual machine
echo ... but let you think for 10 seconds

for i in $(seq 10 -1 0); do echo -n "$i \r"; sleep 1; done;
echo Okay proceeding...

sudo rsync -av --delete-after examples/${CONFIGS}/configs/ /etc/lithos

vagga _build "${CONFIGS}-example"

[ -d /var/lib/lithos/images ] || sudo mkdir -p /var/lib/lithos/images

case $CONFIGS in
    multi_level)
        sudo mkdir -p /var/lib/lithos/images/${CONFIGS}
        sudo rsync -a --delete-after \
            ".vagga/${CONFIGS}-example/" \
            /var/lib/lithos/images/${CONFIGS}/example
        ;;
    *)
        sudo rsync -a --delete-after \
            ".vagga/${CONFIGS}-example/" \
            /var/lib/lithos/images/${CONFIGS}-example
        ;;
esac

echo Done.
echo Ensure that you have run '`vagga make`' before.
echo Then run '`sudo ./target/debug/lithos_tree`' or whatever command you wish

