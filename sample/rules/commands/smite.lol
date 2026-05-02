HAI 1.2
BTW smite -- Thorin attacks the Goblin King with a divine smite
BTW Usage: smite

BTW Select Thorin as the attacker
I IZ RUSTORY_GET_PLAYER YR "Thorin" MKAY
I HAS A STR ITZ I IZ RUSTORY_GET_STAT YR "strength" MKAY
BTW D&D modifier = floor((stat - 10) / 2)
I HAS A MOD ITZ QUOSHUNT OF DIFF OF STR AN 10 AN 2

BTW Roll 1d20 + strength modifier for attack
I HAS A ATK ITZ I IZ RUSTORY_ROLL YR "1d20" MKAY
I HAS A TOTAL ITZ SUM OF ATK AN MOD

BTW Get Goblin King's AC
I IZ RUSTORY_GET_NPC YR "Goblin King" MKAY
I HAS A ENEMY_AC ITZ I IZ RUSTORY_GET_STAT YR "ac" MKAY

BTW Compare: hit if total >= enemy AC
BOTH SAEM TOTAL AN BIGGR OF TOTAL AN ENEMY_AC
O RLY?
  YA RLY
    BTW Hit -- roll 2d8 radiant damage
    I HAS A DMG ITZ I IZ RUSTORY_ROLL YR "2d8" MKAY
    I IZ RUSTORY_DAMAGE YR "Goblin King" AN YR DMG MKAY
    I IZ RUSTORY_DISPLAY YR "SMITE hits! Attack :{TOTAL} vs AC :{ENEMY_AC}. :{DMG} radiant damage!" MKAY
  NO WAI
    BTW Miss
    I IZ RUSTORY_DISPLAY YR "SMITE misses... Attack :{TOTAL} vs AC :{ENEMY_AC}." MKAY
OIC

KTHXBYE
