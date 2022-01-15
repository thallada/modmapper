use anyhow::Result;
use std::fs::File;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
pub async fn main() -> Result<()> {
    let client = reqwest::Client::builder().build()?;
    for z in 10..18 {
        for x in 0..2_u32.pow(z - 9) {
            for y in 0..2_u32.pow(z - 9) {
                sleep(Duration::from_millis(100)).await;
                let url = format!(
                    "https://maps.uesp.net/srmap/color/zoom{z}/skyrim-{x}-{y}-{z}.jpg",
                    z = z,
                    x = x,
                    y = y
                );
                let resp = client.get(&url).send().await?;
                if resp.status().is_success() {
                    println!("{}", url);
                    std::fs::create_dir_all(format!("tiles/{z}/{x}", z = z, x = x))?;
                    let mut out = File::create(format!("tiles/{z}/{x}/{y}.jpg", z = z, x = x, y = y))?;
                    let bytes = resp.bytes().await?;
                    std::io::copy(&mut bytes.as_ref(), &mut out)?;
                }
            }
        }
    }
    Ok(())
}
