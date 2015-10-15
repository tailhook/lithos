======================
Configuration Overview
======================

Lithos has 4 configs:

1. ``/etc/lithos/master.yaml`` -- global configuration for whole
   lithos daemon. Empty config should work most of the time.
   :ref:`master_config`
2. ``/etc/lithos/sandboxes/<NAME>.yaml`` -- the allowed paths and other system
   limits for every sandbox. You may think of a sandbox as a single
   application.
   :ref:`sandbox_config`
3. ``/etc/lithos/processes/<NAME>.yaml`` -- you may think of it as a list of
   pairs (image_name, num_of_processes_to_run). It's only a tiny bit
   longer than that.
   :ref:`process_config`
4. ``<IMAGE>/config/<NAME>.yaml`` -- configuration of process to run. It's
   where all the needed to run process are. It's stored inside the image (so
   updated with new image), and limited by limits in sandbox config.
   :ref:`container_config`

It may look too much. But note that in some real-world deployment I have first
two configs contain 8 lines (5 unique settings). The third is simple.
And the fourth has essential info you need to run process in like in any other
supervisor.

