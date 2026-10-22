# Builds the crawl oracle's harness against a crawl build. Run from crawl-ref/source with the
# flags crawl was built with, so make reuses its objects:
#
#   make -f Makefile -f <this file> WEBTILES=y USE_DGAMELAUNCH=y crawl-oracle-melee
#
# The harness takes main.o's place, as crawl's util/monster does.

ORACLE_DIR := $(dir $(lastword $(MAKEFILE_LIST)))

crawl-oracle-melee.o: $(ORACLE_DIR)melee.cc $(ORACLE_DIR)common.h | $(GENERATED_HEADERS) $(TILEDEFHDRS)
	$(CXX) $(STDFLAG) $(ALL_CFLAGS) -fno-access-control -c $< -o $@

crawl-oracle-melee: crawl-oracle-melee.o $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)
	$(CXX) $(LDFLAGS) crawl-oracle-melee.o $(OBJECTS) $(EXTRA_OBJECTS) -o $@ $(LIBS)

.PHONY: crawl-oracle-objects
# Crawl's own objects, without the harness: what CI caches.
crawl-oracle-objects: $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)

# The spell harness includes mon-cast.cc for its static spell choice, so it links without
# mon-cast.o.
crawl-oracle-spells.o: $(ORACLE_DIR)spells.cc $(ORACLE_DIR)common.h | $(GENERATED_HEADERS) $(TILEDEFHDRS)
	$(CXX) $(STDFLAG) $(ALL_CFLAGS) -fno-access-control -c $< -o $@

crawl-oracle-spells: crawl-oracle-spells.o $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)
	$(CXX) $(LDFLAGS) crawl-oracle-spells.o $(filter-out mon-cast.o,$(OBJECTS)) $(EXTRA_OBJECTS) -o $@ $(LIBS)

# The missile harness includes mon-act.cc for its static _handle_wand, so it links without
# mon-act.o.
crawl-oracle-missiles.o: $(ORACLE_DIR)missiles.cc $(ORACLE_DIR)common.h | $(GENERATED_HEADERS) $(TILEDEFHDRS)
	$(CXX) $(STDFLAG) $(ALL_CFLAGS) -fno-access-control -c $< -o $@

crawl-oracle-missiles: crawl-oracle-missiles.o $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)
	$(CXX) $(LDFLAGS) crawl-oracle-missiles.o $(filter-out mon-act.o,$(OBJECTS)) $(EXTRA_OBJECTS) -o $@ $(LIBS)

# The turn harness includes mon-act.cc, to count what a monster does as it calls into the code
# that does it, so it links without mon-act.o.
crawl-oracle-turns.o: $(ORACLE_DIR)turns.cc $(ORACLE_DIR)common.h | $(GENERATED_HEADERS) $(TILEDEFHDRS)
	$(CXX) $(STDFLAG) $(ALL_CFLAGS) -fno-access-control -c $< -o $@

crawl-oracle-turns: crawl-oracle-turns.o $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)
	$(CXX) $(LDFLAGS) crawl-oracle-turns.o $(filter-out mon-act.o,$(OBJECTS)) $(EXTRA_OBJECTS) -o $@ $(LIBS)

crawl-oracle-player.o: $(ORACLE_DIR)player.cc $(ORACLE_DIR)common.h | $(GENERATED_HEADERS) $(TILEDEFHDRS)
	$(CXX) $(STDFLAG) $(ALL_CFLAGS) -fno-access-control -c $< -o $@

crawl-oracle-player: crawl-oracle-player.o $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)
	$(CXX) $(LDFLAGS) crawl-oracle-player.o $(OBJECTS) $(EXTRA_OBJECTS) -o $@ $(LIBS)

crawl-oracle-speed.o: $(ORACLE_DIR)speed.cc $(ORACLE_DIR)common.h | $(GENERATED_HEADERS) $(TILEDEFHDRS)
	$(CXX) $(STDFLAG) $(ALL_CFLAGS) -fno-access-control -c $< -o $@

crawl-oracle-speed: crawl-oracle-speed.o $(OBJECTS) $(EXTRA_OBJECTS) $(CONTRIB_LIBS)
	$(CXX) $(LDFLAGS) crawl-oracle-speed.o $(OBJECTS) $(EXTRA_OBJECTS) -o $@ $(LIBS)
