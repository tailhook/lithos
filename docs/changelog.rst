=========================
Lithos Changes By Release
=========================


.. _changelog 0.18.3:

v0.18.3
=======

* Bugfix: it looks like that reading through ``/proc/`` is inherently racy,
  i.e. some process may be skipped. This commit fixes walk faster and traverse
  directory twice. More elaborate fix will be implemented in future.


.. _changelog 0.18.2:

v0.18.2
=======

* Feature: add ``secret-environ-file`` which can be used to offload secrets
  to a separate (perhaps shared) file

.. _changelog 0.18.1:

v0.18.1
=======

* Feature: add ``set-non-block`` option to tcp-ports


.. _changelog 0.18.0:

v0.18.0
=======

* Breaking: we don't run ``arping`` after container setup by default,
  as it `doesn't work in certain environments`__.
  Use :bopt:`after-setup-command` instead.

__ https://github.com/tailhook/lithos/issues/17

.. _changelog 0.17.8:

v0.17.8
=======

* Bugfix: fixes issue with bridged networking when host system
  is alpine (`#15`__)

__ https://github.com/tailhook/lithos/issues/15


.. _changelog 0.17.7:

v0.17.7
=======

* Bugfix: log name of the process when lithos_knot failed
* Bugfix: more robust parsing of process names by lithos_ps
* Feature: add ``@{lithos:pid}`` magic variable


.. _changelog 0.17.6:

v0.17.6
=======

* Bugfix: systemd protocol support fixed: LISTEN_FDNAMES and LISTEN_PID


.. _changelog 0.17.5:

v0.17.5
=======

* Feature: check variable substitution with ``lithos_check`` even in
  ``--check-container`` (out of system) mode

.. _changelog 0.17.4:

v0.17.4
=======

* Feature: Add ``DottedName`` :ref:`variable type <container_variables>`
* Feature: Add ``activation`` parameter to ``TcpPort`` variable


.. _changelog 0.17.3:

v0.17.3
=======

* Bugfix: fix EADDRINUSE error when all children requiring file descriptor
  where queued for restart (throttled), bug was due to duped socket lying in
  scheduled command (where main socket is closed to notify peers there are
  no listeners)


.. _changelog 0.17.2:

v0.17.2
=======

* Bugfix: previously lithos_tree process after fork but before execing
  lithos_knot could be recognized as undefined child and killed.
  This race-condition sometimes led to closing sockets prematurely and being
  unable to listen them again


.. _changelog 0.17.1:

v0.17.1
=======

* Bugfix: passing sockets as FDs in non-bridged network was broken in v0.17.0


.. _changelog 0.17.0:

v0.17.0
=======

* Breaking: add ``external`` flag to :opt:`tcp-ports`, which by default is
  ``false`` (previous behavior was equal to ``external: true``)
* Bugfix: ``lithos_cmd`` now returns exit code 0 if underlying command is
  exited successfully (was broken in 0.15.5)


.. _changelog 0.16.0:

v0.16.0
=======

* Breaking: remove ``v1`` encryption for secrets (it was alive for a week)
* Feature: add :opt:`secrets-namespaces` and :opt:`extra-secrets-namespaces`
  option to allow namespacing secrets on top of a single key
* Feature: add ``v2`` key encryption scheme


.. _changelog 0.15.6:

v0.15.6
=======

* Feature: add :opt:`secret-environ` and :opt:`secrets-private-key`` settings
  which allow to pass to the application decrypted environment variables
* Bugfix: when bridged network is enabled we use ``arping`` to update ARP cache


.. _changelog 0.15.5:

v0.15.5
=======

* Bugfix: add support for bridged-network and ip-addresses for lithos_cmd
* Bugfix: initialize looppack interface in container when ``bridged-network``
  is configured
* Feature: allow ``lithos_cmd`` without ``ip_addresses`` (only loopback is
  initialized in this case)
* Bugfix: return error result from ``lithos_cmd`` if inner process failed


.. _changelog 0.15.4:

v0.15.4
=======

* First release that stops support of ubuntu precise and
  adds repository for ubuntu bionic
* Bugfix: passing TCP port as fd < 3 didn't work before, now we allow ``fd: 0``
  and fail gracefully on 1, 2.


.. _changelog 0.15.3:

v0.15.3
=======

* feature: Add :opt:`default-user` and :opt:`default-group` to simplify
  container config
* bugfix: fix containers having symlinks at ``/etc/{resolv.conf, hosts}``
  (broken in v0.15.0)

.. _changelog 0.15.2:

v0.15.2
=======

* bugfix: containers without bridged network work again


.. _changelog 0.15.1:

v0.15.1
=======

* nothing changed, fixed tests only

.. _changelog 0.15.0:

v0.15.0
=======

* feature: Add :opt:`normal-exit-codes` setting
* feature: Add :opt:`resolv-conf` and :opt:`hosts-file` to sandbox config
* feature: Add :opt:`bridged-network` option to sandbox config
* breaking: By default ``/etc/hosts`` and ``/etc/resolv.conf`` will be mounted
  if they are proper mount points (can be opt out in container config)


.. _changelog 0.14.3:

v0.14.3
=======

* Bugfix: when more than one variable is used lithos were restarting process
  every time (because of unstable serialization of hashmap)


.. _changelog 0.14.2:

v0.14.2
=======

* Bugfix: if ``auto-clean`` is different in several sandboxes looking at the
  same image directory we skip cleaning the dir and print a warning
* Add a timestamp to ``lithos_clean`` output (in ``--delete-unused`` mode)

.. _changelog 0.14.1:

v0.14.1
=======

* Bugfix: variable substitution was broken in v0.14.0


.. _changelog 0.14.0:

v0.14.0
=======

* Sets ``memory.memsw.limit_in_bytes`` if that exists (usually requires
  ``swapaccount=1`` in kernel params).
* Adds a warning-level message on process startup
* Duplicates startup and death messages into stderr log, so you can corelate
  them with application messages


.. _changelog 0.13.2:

v0.13.2
=======

* Upgrades many dependencies, no significant changes or bugfixes


.. _changelog 0.13.1:

v0.13.1
=======

* Adds :opt:`auto-clean` setting


.. _changelog 0.13.0:

v0.13.0
=======

* ``/dev/pts/ptmx`` is created with ``ptmxmode=0666``, which makes it suitable
  for creating ptys by unprivileged users. We always used ``newinstance``
  option, so it should be safe enough. And it also matches how ``ptmx`` is
  configured on most systems by default

.. _changelog 0.12.1:

v0.12.1
=======

* Added ``image-dir-levels`` parameter which allows using images in
  form of ``xx/yy/zz`` (for value of ``3``) instead of bare name

.. _changelog 0.12.0:

v0.12.0
=======

* Fixed order of ``sandbox-name.process-name`` in metrics
* Dropped setting ``cantal-appname`` (never were useful, because cantal
  actually uses cgroup name, and lithos master process actually has one)

.. _changelog 0.11.0:

v0.11.0
=======

* Option :opt:`cantal-appname` added to a config
* If no ``CANTAL_PATH`` present in environment we set it to some default,
  along with ``CANTAL_APPNAME=lithos`` unless :opt:`cantal-appname` is
  overriden.
* Added default container environment ``LITHOS_CONFIG``. It may be used to
  log config name, read metadata and other purposes.


.. _changelog 0.10.7:

v0.10.7
=======

* Cantal_ metrics added

.. _cantal: https://cantal.readthedocs.io
