.. _volumes:

=======
Volumes
=======

Volumes in lithos are just some kind of mount-points. The mount points are not
created by ``lithos`` itself. So they must exist either in original image. Or
on respective volume (if mount point is inside a volume).

There are the following kinds of volumes:

``!Readonly "/path/to/dir"``
    A **read-only** bind mount for some dir. The directory is mounted with
    ``ro,nosuid,noexec,nodev``

``!Persistent { path: /path/to/dir, mkdir: false, mode: 0o700, user: 0, group: 0 }``
    A **writeable** bind mount. The directory is mounted with
    ``rw,nosuid,noexec,nodev``. If you need directory to be created set
    ``mkdir`` to ``true``. You also probably need to customize either the user
    (to the one running command e.g. same as ``user-id`` of the container) or
    the mode (to something like ``0o1777``, i.e. sticky writable by anyone).

``!Statedir { path: /, mode: 0o700, user: 0, group: 0 }``
    Mount subdir of the container's own state directory. This directory is
    used to store generated ``resolv.conf`` and ``hosts`` files as well as for
    other kinds of small state which is dropped when container dies. If you
    mount something other than ``/`` you should custimize mode or an owner
    similarly to ``!Persistent`` volumes (except that you can't create statedir
    subdirectory by hand because statedir is created for each process at start)

``!Tmpfs { size: 100Mi, mode: 0o766 }``
    The tmpfs mount point. Currently only ``size`` and ``mode`` options
    supported. Note that syntax of size and mode is generic syntax for
    numbers for our configuration library, not the syntax supported by kernel.
