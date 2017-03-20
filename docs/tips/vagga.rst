==========================
Deploying Vagga Containers
==========================

Vagga_ is a common way to develop applications for later deployment using
lithos. Also vagga is a common way to prepare a container image for use with
lithos.

Usually vagga_ does it's best to make containers as close to production as
possible. Still vagga tries to make good trade-off to make it's easier to
use for development, so there are few small quircks that you may or may not
notice when deploying.

Here is a boring list, later sections describe some things in more detail:

1. Unsurprisingly ``/work`` directory is absent in production container.
   Usually this means three things:

   a. Your sources must be copied/installed into container (e.g. using Copy_)
   b. There is no current working directory, unless you specify it explicitly
      current directory is root ``/``
   c. You can't **write** into working directory or ``/work/somewhere``

2. All directories are read-only by default. Basic consequences are:

   a. There is no writable ``/tmp`` unless you specify one. This also means
      there is no default for temporary dir, you have to chose whether this
      is an in-memory :volume:`Tmpfs` or on-disk :volume:`Persistent`.
   b. There is no ``/dev/shm`` by default. This is just another ``tmpfs``
      volume in every system nowadays, so just measure how much you need and
      mount a :volume:`Tmpfs`. Be aware that each container even on same
      machine get's it's own instance.
   c. We can't even overwrite ``/etc/resolv.conf`` and ``/etc/hosts``, see
      below.

3. There are few environment variables that vagga sets in container by default:

   a. ``TERM`` -- is propagated from external environment. For daemons it
      should never matter. For :opt:`interactive` commands it may matter.
   b. ``PATH`` -- in vagga is set to hard-coded value. There is no default
      value in lithos. If your program runs any binaries (and usually lots of
      them do, even if you don't expect), you want to set ``PATH``.
   c. Various ``*_proxy`` variables are propagated. They are almost never
      useful for daemons. But are written here for completeness.


4. In vagga we don't update ``/etc/resolv.conf`` and ``/etc/hosts``, but in
   lithos we have such mechanism. The mechanism is following:

   a. In container you make the symlinks
      ``/etc/resolv.conf -> /state/resolv.conf``,
      ``/etc/hosts -> /state/hosts``
   b. The ``/state`` directory is mounted as :volume:`Statedir`
   c. Lithos automatically puts ``resolv.conf`` and ``hosts`` into statedir
      when container is created (respecting :opt:`resolv-conf`
      and :opt:`hosts-file`)
   d. Then files can be updated by updating files
      in ``/var/run/lithos/state/<sandbox>/<process>/``

5. Because by default neither vagga nor lithos have network isolation, some
   things that are accessible in the dev system may not be accessible in the
   server system. This includes both, services on ``localhost`` as well as
   in **abstract unix socket namespace**. Known examples are:

   a. Dbus: for example if ``DBUS_SESSION_BUS_ADDRESS`` starts with
      ``unix:abstract=``
   b. Xorg: X Window System, the thing you configure with ``DISPLAY``
   c. nscd: name service cache daemon (this thing may resolve DNS names even
      if TCP/IP network is absent for your container)
   d. systemd-resolved: listens at ``127.0.0.53:53`` as well as on **dbus**

.. _vagga: http://vagga.readthedocs.io/en/latest/
.. _copy: http://vagga.readthedocs.io/en/latest/build_steps.html?highlight=Copy#step-Copy
