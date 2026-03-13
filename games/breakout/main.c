#include <gb/gb.h>
#include <gb/drawing.h>
#include <string.h>
#include <stdio.h>

// Screen: 160x144 pixels, 20x18 tiles (8x8 each)

// Paddle
#define PADDLE_Y      136
#define PADDLE_WIDTH  24
#define PADDLE_HEIGHT 4
#define PADDLE_SPEED  3

// Ball
#define BALL_SIZE     4
#define BALL_SPEED    2

// Bricks
#define BRICK_ROWS    5
#define BRICK_COLS    8
#define BRICK_W       18
#define BRICK_H       8
#define BRICK_START_X 4
#define BRICK_START_Y 16

// Game state
UINT8 paddle_x;
UINT8 ball_x, ball_y;
INT8 ball_dx, ball_dy;
UINT8 bricks[BRICK_ROWS][BRICK_COLS];
UINT8 score;
UINT8 lives;
UINT8 bricks_left;
UINT8 game_over;

void draw_paddle(void) {
    color(DKGREY, WHITE, SOLID);
    box(paddle_x, PADDLE_Y, paddle_x + PADDLE_WIDTH, PADDLE_Y + PADDLE_HEIGHT, M_FILL);
}

void clear_paddle(void) {
    color(WHITE, WHITE, SOLID);
    box(paddle_x, PADDLE_Y, paddle_x + PADDLE_WIDTH, PADDLE_Y + PADDLE_HEIGHT, M_FILL);
}

void draw_ball(void) {
    color(BLACK, WHITE, SOLID);
    box(ball_x, ball_y, ball_x + BALL_SIZE, ball_y + BALL_SIZE, M_FILL);
}

void clear_ball(void) {
    color(WHITE, WHITE, SOLID);
    box(ball_x, ball_y, ball_x + BALL_SIZE, ball_y + BALL_SIZE, M_FILL);
}

void draw_brick(UINT8 row, UINT8 col) {
    UINT8 x = BRICK_START_X + col * (BRICK_W + 1);
    UINT8 y = BRICK_START_Y + row * (BRICK_H + 1);

    // Different shades per row
    if (row == 0) color(BLACK, WHITE, SOLID);
    else if (row == 1) color(DKGREY, WHITE, SOLID);
    else if (row == 2) color(LTGREY, WHITE, SOLID);
    else if (row == 3) color(DKGREY, WHITE, SOLID);
    else color(BLACK, WHITE, SOLID);

    box(x, y, x + BRICK_W, y + BRICK_H, M_FILL);
}

void clear_brick(UINT8 row, UINT8 col) {
    UINT8 x = BRICK_START_X + col * (BRICK_W + 1);
    UINT8 y = BRICK_START_Y + row * (BRICK_H + 1);
    color(WHITE, WHITE, SOLID);
    box(x, y, x + BRICK_W, y + BRICK_H, M_FILL);
}

void draw_hud(void) {
    char buf[18];
    color(BLACK, WHITE, SOLID);
    sprintf(buf, "SCORE:%d LIVES:%d", (int)score, (int)lives);
    gotogxy(0, 0);
    gprint(buf);
}

void init_bricks(void) {
    UINT8 r, c;
    bricks_left = BRICK_ROWS * BRICK_COLS;
    for (r = 0; r < BRICK_ROWS; r++) {
        for (c = 0; c < BRICK_COLS; c++) {
            bricks[r][c] = 1;
            draw_brick(r, c);
        }
    }
}

void reset_ball(void) {
    ball_x = paddle_x + PADDLE_WIDTH / 2 - BALL_SIZE / 2;
    ball_y = PADDLE_Y - BALL_SIZE - 2;
    ball_dx = BALL_SPEED;
    ball_dy = -BALL_SPEED;
}

void init_game(void) {
    score = 0;
    lives = 3;
    game_over = 0;
    paddle_x = 68; // Center

    // Clear screen
    color(WHITE, WHITE, SOLID);
    box(0, 0, 159, 143, M_FILL);

    init_bricks();
    reset_ball();
    draw_paddle();
    draw_ball();
    draw_hud();
}

void check_brick_collision(void) {
    UINT8 r, c;
    UINT8 bx, by;

    for (r = 0; r < BRICK_ROWS; r++) {
        for (c = 0; c < BRICK_COLS; c++) {
            if (!bricks[r][c]) continue;

            bx = BRICK_START_X + c * (BRICK_W + 1);
            by = BRICK_START_Y + r * (BRICK_H + 1);

            // AABB collision
            if (ball_x + BALL_SIZE > bx && ball_x < bx + BRICK_W &&
                ball_y + BALL_SIZE > by && ball_y < by + BRICK_H) {

                bricks[r][c] = 0;
                clear_brick(r, c);
                ball_dy = -ball_dy;
                score++;
                bricks_left--;
                draw_hud();
                return;
            }
        }
    }
}

void update(void) {
    UINT8 keys = joypad();

    // Move paddle
    clear_paddle();
    if ((keys & J_LEFT) && paddle_x >= PADDLE_SPEED) {
        paddle_x -= PADDLE_SPEED;
    }
    if ((keys & J_RIGHT) && paddle_x <= 160 - PADDLE_WIDTH - PADDLE_SPEED) {
        paddle_x += PADDLE_SPEED;
    }
    draw_paddle();

    // Move ball
    clear_ball();

    // Wall collisions — use INT16 for safe signed math
    {
        INT16 bx = (INT16)ball_x + ball_dx;
        INT16 by = (INT16)ball_y + ball_dy;

        if (bx < 1) { bx = 1; ball_dx = -ball_dx; }
        if (bx > 160 - BALL_SIZE) { bx = 160 - BALL_SIZE; ball_dx = -ball_dx; }
        if (by < 9) { by = 9; ball_dy = -ball_dy; }

        ball_x = (UINT8)bx;
        ball_y = (UINT8)by;
    }

    // Paddle collision
    if (ball_dy > 0 &&
        ball_y + BALL_SIZE >= PADDLE_Y &&
        ball_y + BALL_SIZE <= PADDLE_Y + PADDLE_HEIGHT + 4 &&
        ball_x + BALL_SIZE > paddle_x &&
        ball_x < paddle_x + PADDLE_WIDTH) {

        ball_dy = -ball_dy;
        ball_y = PADDLE_Y - BALL_SIZE;

        // Angle based on hit position
        {
            UINT8 hit_pos = ball_x + BALL_SIZE / 2 - paddle_x;
            if (hit_pos < PADDLE_WIDTH / 3) {
                ball_dx = -BALL_SPEED;
            } else if (hit_pos > PADDLE_WIDTH * 2 / 3) {
                ball_dx = BALL_SPEED;
            }
        }
    }

    // Bottom — lose life
    if (ball_y >= 140) {
        lives--;
        draw_hud();
        if (lives == 0) {
            game_over = 1;
        } else {
            reset_ball();
        }
    }

    // Brick collision
    check_brick_collision();

    // Win check
    if (bricks_left == 0) {
        game_over = 2; // Win
    }

    draw_ball();
}

void show_title(void) {
    color(WHITE, WHITE, SOLID);
    box(0, 0, 159, 143, M_FILL);

    color(BLACK, WHITE, SOLID);
    gotogxy(4, 6);
    gprint("OXIBOY BREAKOUT");
    gotogxy(3, 10);
    gprint("PRESS START");

    // Wait for START
    waitpad(J_START);
    waitpadup();
}

void show_end(UINT8 won) {
    char buf[18];
    color(WHITE, WHITE, SOLID);
    box(0, 40, 159, 100, M_FILL);

    color(BLACK, WHITE, SOLID);
    if (won) {
        gotogxy(4, 7);
        gprint("YOU WIN!");
    } else {
        gotogxy(3, 7);
        gprint("GAME OVER");
    }
    sprintf(buf, "SCORE: %d", (int)score);
    gotogxy(5, 9);
    gprint(buf);
    gotogxy(2, 11);
    gprint("PRESS START");

    waitpad(J_START);
    waitpadup();
}

void main(void) {
    // Use drawing mode
    mode(M_DRAWING);

    while (1) {
        show_title();
        init_game();

        while (!game_over) {
            update();
            wait_vbl_done(); // ~60fps sync
        }

        show_end(game_over == 2);
    }
}
