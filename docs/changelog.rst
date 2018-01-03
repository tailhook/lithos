=========================
Lithos Changes By Release
=========================


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
