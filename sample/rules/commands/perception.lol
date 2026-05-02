HAI 1.2
BTW perception -- Make a Wisdom-based perception check
BTW Usage: perception

BTW Select Thorin as the character making the check
I IZ RUSTORY_GET_PLAYER YR "Thorin" MKAY

BTW Run an ability check using wisdom with DC 15
I HAS A RESULT ITZ I IZ RUSTORY_CHECK YR "ability_check" AN YR "ability" AN YR "wisdom" AN YR "dc" AN YR "15" MKAY

BTW Check the result
BOTH SAEM RESULT AN "success"
O RLY?
  YA RLY
    I IZ RUSTORY_DISPLAY YR "Thorin's perception succeeds! You notice something hidden..." MKAY
    BTW Tag the character as having spotted something
    I IZ RUSTORY_ADD_TAG YR "perceptive" MKAY
  NO WAI
    BOTH SAEM RESULT AN "critical"
    O RLY?
      YA RLY
        I IZ RUSTORY_DISPLAY YR "Critical perception! Thorin sees everything clearly." MKAY
        I IZ RUSTORY_ADD_TAG YR "perceptive" MKAY
      NO WAI
        I IZ RUSTORY_DISPLAY YR "Thorin fails the perception check. Nothing seems out of place." MKAY
    OIC
OIC

KTHXBYE
