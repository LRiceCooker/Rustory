use std::{panic};
pub struct Roll {
    pub dice: i8, 
    pub value: i8, 
    pub add: i16
}

pub fn parse(args: Vec<&str>) -> Result<Roll, ()>{
    let matchable_result = panic::catch_unwind(|| {
        let extarcted_param = args[0];
        let parsed_roll: Roll;
        let mut dice: i8 = 0;
        let mut value: i8 = 0; 
        let mut add: i16 = 0;
        //assert!(extarcted_param.contains('d'));
        let splited_param = extarcted_param.split('d').collect::<Vec<&str>>();
        //assert!(splited_param[0].parse::<i8>().is_ok());
        dice = splited_param[0].parse::<i8>().expect("");
        //assert!(splited_param.len()==2);
        if splited_param[1].parse::<i8>().is_ok() {
            value = splited_param[1].parse::<i8>().expect("");
            add = 0;
        }
        else {
            //assert!(splited_param[1].contains('+') || splited_param[1].contains('-'));
            let value_and_bonnus: Vec<&str>;//ternary + modifier sign
            let mut operator = "";
            if splited_param[1].contains('+') {
                value_and_bonnus = splited_param[1].split('+').collect::<Vec<&str>>();
            }
            else {
                value_and_bonnus = splited_param[1].split('-').collect::<Vec<&str>>();
                operator = "-";
            }
            //assert!(value_and_bonnus[0].parse::<i8>().is_ok());
            value = value_and_bonnus[0].parse::<i8>().expect("");
            //assert!(value_and_bonnus[1].parse::<i16>().is_ok());
            println!("{}", operator.to_string()+value_and_bonnus[1]);
            add = (operator.to_string()+value_and_bonnus[1]).parse::<i16>().expect("");
        }
        
        return Roll {dice, value, add}
        
    });

    match matchable_result {
        Ok(result) => Ok(result), 
        Err(_) => Err(())
    }

}