// Used only during testing to generate samples for the parser component
// We're at the mercy of the system's C compiler but this seems to be a better tradeoff than 
// trying to translate these into Rust syntax and hoping we're not run-over by any quirks.
// It's also a lot easier to do because, crucially:
// LLM SLOP PRESENCE: ABSURD
#include <stdio.h>
#include <stdint.h>
#include <string.h>

// Helper to write a file
void write_file(const char* filename, uint8_t (*func)(int)) {
    FILE *f = fopen(filename, "wb");
    if (!f) {
        fprintf(stderr, "Failed to open output file %s\n", filename);
        return;
    }
    for (int t = 0; t < 65536; t++) {
        uint8_t s = func(t);
        fwrite(&s, 1, 1, f);
    }
    fclose(f);
    printf("Generated %s\n", filename);
}

// 1. "the 42 melody"
uint8_t melody_42(int t) {
    return t*(42&t>>10);
}

// 2. "Neurofunk"
uint8_t neurofunk(int t) {
    return t*((t&4096?t%65536<59392?7:t&7:16)+(1&t>>14))>>(3&-t>>(t&2048?2:10))|t>>(t&16384?t&4096?10:3:2);
}

// 3. "chip"
uint8_t chip(int t) {
    return (t&1024||t&16384&&t&2048&&!(t&512))?(t&4096&&!(t&2048)?(t*t*t>>~t*t)+127:t*((t>>11&1)+1)*(1+(t>>16&1)*3))*2:0;
}

// 4. "Bytebreak"
uint8_t bytebreak(int t) {
    return ((t&32767)>>13==2|(t&65535)>>12==9?(t^-(t/8&t>>5)*(t/8&127))&(-(t>>5)&255)*((t&65535)>>12==9?2:1):(t&8191)%((t>>5&255^240)==0?1:t>>5&255^240))/4*3+(t*4/(4+(t>>15&3))&128)*(-t>>11&2)*((t&32767)>>13!=2)/3;
}

// 5. "Wheezing modem"
uint8_t wheezing_modem(int t) {
    return 100*((t<<2|t>>5|t^63)&(t<<10|t>>11));
}

// 6. "Electrohouse"
uint8_t electrohouse(int t) {
    return t>>(((t%2?t%((t>>13)%8>=2?((t>>13)%8>=4?41:51):61):t%34)))|(~t>>4);
}

// 7. "THE HIT OF THE SEASON"
uint8_t hit_of_the_season(int t) {
    return (t>0&t<65535?t%32>(t/10000)?t>>4:t>>6:0)&(t>>4);
}

int main() {
    write_file("reference_42_melody.bin", melody_42);
    write_file("reference_neurofunk.bin", neurofunk);
    write_file("reference_chip.bin", chip);
    write_file("reference_bytebreak.bin", bytebreak);
    write_file("reference_wheezing_modem.bin", wheezing_modem);
    write_file("reference_electrohouse.bin", electrohouse);
    write_file("reference_hit_of_the_season.bin", hit_of_the_season);
    return 0;
}
