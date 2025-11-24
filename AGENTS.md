Read through the makefile first. Strongly prefer to do things via existing makefile commands rather than by manually running docker, cargo, etc. If there isn't a makefile command for the thing that you want to do, consider making one.

The only dev dependencies should be docker and make. Everything else should run inside docker containers.
