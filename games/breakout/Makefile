GBDK = /home/ubuntu/gbdk
LCC = $(GBDK)/bin/lcc
ROM_NAME = breakout.gb

all: $(ROM_NAME)

$(ROM_NAME): main.c
	$(LCC) -Wa-l -Wl-m -Wl-j -o $@ $<

clean:
	rm -f *.o *.lst *.map *.gb *.ihx *.sym *.cdb *.adb *.asm

.PHONY: all clean
