=========================
Lithos Changes By Release
=========================


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
