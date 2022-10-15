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
        let splited_param = extarcted_param.split('d').collect::<Vec<&str>>();
        dice = splited_param[0].parse::<i8>().expect("");
        if splited_param[1].parse::<i8>().is_ok() {
            value = splited_param[1].parse::<i8>().expect("");
            add = 0;
        }
        else {
            let value_and_bonnus: Vec<&str>;
            let mut operator = "";
            if splited_param[1].contains('+') {
                value_and_bonnus = splited_param[1].split('+').collect::<Vec<&str>>();
            }
            else {
                value_and_bonnus = splited_param[1].split('-').collect::<Vec<&str>>();
                operator = "-";
            }
            value = value_and_bonnus[0].parse::<i8>().expect("");
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