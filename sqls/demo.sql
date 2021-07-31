--? age: num = 18 // 最低年龄
select
    name, age
from student
where age >= @age