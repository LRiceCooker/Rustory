HAI 1.2
BTW heal -- Restore hit points to a wounded ally
BTW Usage: heal

BTW Select Thorin as the healer
I IZ RUSTORY_GET_PLAYER YR "Thorin" MKAY

BTW Check if healer has spell slots available
I HAS A SLOTS ITZ I IZ RUSTORY_GET_POOL YR "spell_slots" MKAY
BOTH SAEM SLOTS AN BIGGR OF SLOTS AN 1
O RLY?
  YA RLY
    BTW Spend one spell slot
    I IZ RUSTORY_SPEND YR "spell_slots" AN YR 1 MKAY

    BTW Roll 1d8 healing
    I HAS A HEAL_AMT ITZ I IZ RUSTORY_ROLL YR "1d8" MKAY

    BTW Get current HP before healing
    I HAS A HP_BEFORE ITZ I IZ RUSTORY_GET_GAUGE YR "hp" MKAY

    BTW Apply healing to self
    I IZ RUSTORY_HEAL YR "Thorin" AN YR HEAL_AMT MKAY

    BTW Get HP after healing
    I HAS A HP_AFTER ITZ I IZ RUSTORY_GET_GAUGE YR "hp" MKAY

    I HAS A SLOTS_LEFT ITZ I IZ RUSTORY_GET_POOL YR "spell_slots" MKAY
    I IZ RUSTORY_DISPLAY YR "Healed Thorin for :{HEAL_AMT} HP (:{HP_BEFORE} -> :{HP_AFTER}). Spell slots remaining:: :{SLOTS_LEFT}" MKAY
  NO WAI
    I IZ RUSTORY_DISPLAY YR "No spell slots remaining!" MKAY
OIC

KTHXBYE
