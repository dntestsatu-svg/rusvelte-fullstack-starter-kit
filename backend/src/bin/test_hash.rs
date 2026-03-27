use backend::infrastructure::security::argon2::hash_password;

fn main() {
    let password = "dev12345";
    match hash_password(password) {
        Ok(hash) => println!("{}", hash),
        Err(e) => eprintln!("Error: {}", e),
    }
}
