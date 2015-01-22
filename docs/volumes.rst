.. _volumes:

=======
Volumes
=======

Volumes in lithos are just some kind of mount-points. The mount points are not
created by ``lithos`` itself. So they must exist either in original image. Or
on respective volume (if mount point is inside a volume).

There are the following
kinds of volumes:

``/path/to/dir``
    A **read-only** bind mount for some dir. The directory is mounted with
    ``ro,nosuid,noexec,nodev``

``rw:/path/to/dir``
    A **writeable** bind mount. The directory is mounted with
    ``rw,nosuid,noexec,nodev``

``state:/``
    Mount subdir of the container's own state directory. This directory is
    used to store generated ``resolv.conf`` and ``hosts`` files as well as for
    other kinds of small state which is dropped when container dies.

``tmpfs:size=1G,mode=1777``
    The tmpfs mount point. Currently all options after colon ``:`` are passed
    to the kernel as is. We will impose some limits and validation later.
