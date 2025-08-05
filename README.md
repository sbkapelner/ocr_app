This is a web application written in Rust for cross checking reference numerals in patent figures with reference numerals in the written description. The reference numerals are extracted from the patent figures using fine-tuned versions of the OCRS models and OCR engine by Robert Knight. See https://github.com/robertknight/ocrs-models and https://github.com/robertknight/ocrs.

Try out the beta version here: https://ocr-app-1058728175963.northamerica-northeast1.run.app/

## Introduction
I built some software that is still in its early stages and I'm looking to present it to you all and get some feedback since the target users are patent practitioners. And I want to note that this is a personal project of mine and it didn't involve any firm input, or firm resources etc. There's nothing official about it. So the software performs cross checking of the reference numerals in your patent application figures and reference numerals in your written description. It runs in the browser and you upload a pdf set of figures and a word document written description via the browser. And even though it runs in the browser if we host it on a private firm server it is of course private. And most of my motivation for making this project is rooted in the fact that we can't send client data to an API hosted by say Amazon or Google Cloud. 

- OCR stands for optical character recognition. The most common application is extracting text from images. For example, taking a picture of a check and being able to extract the account number and the routing number is a simple example.
- Patent figures are much more difficult to OCR than say a check because the numbers are scattered throughout, they do not appear in a structured way and the surrounding drawing content makes them difficult to detect. 
- To use this software with client data I need an open source OCR model. I can't use Amazon or Google Cloud's OCR API, or even Chat GPT.
- All of the open source OCR models I have tried are not good, at least for detecting reference numerals in patent figures. 

> - I fine tuned two models for this use case using PyTorch (A deep learning framework)

- OCR usually involves two separate models: one for detection, i.e., where is the text in the image and one one for recognition, i.e., what is the text in the image, after we have found some text what does that text say.

## Benefits
- This software is private if hosted on a private server
- The models I fine-tuned are only 3Mb and 9Mb so they could even be packaged with the software and run on everyone's individual laptops if we don't want to host the app on a server. 
> - I used Robert Knight's OCR models and engine.
> - The front end UI, the web server, and the OCR Engine (the piece of software that ingests the image and uses the models to perform the ocr) are written in Rust.
- The cost to use is free. This was not free for me to build. I spent like $300 making my dataset and then renting out compute resources to do the fine-tuning but its free for all of you. 
- Every time you use an Amazon or Google Cloud OCR service you have to pay ($0.001 per image). 
- The models were fine-tuned with a dataset that was built using the Amazon Rekognition API, which is has a state of the art OCR model. So the goal was to make this Amazon level quality for patent drawings and for free and private.

## Dataset
- Patent figures from the USPTO website
- size:
	- training: 49,208 images
	- validation: 12,303 images
	- total: ~61,000 images

## Training
#### Detection
- Loss Function (Balance Cross-Entropy Loss)
	- False negatives are highly penalized during training. 10x more than false positives.
- Low detection threshold
	- And we also favor recall by setting a low detection threshold because I want to ensure we catch all text regions and do not care about false positives as much
- Focal Loss
	- In OCR non-text regions (negative samples) are much more numerous than text regions (+ samples) so text regions or pixels are weighted more heavily
	- If we incorrectly detect a non-text region with high confidence we have a high focal loss
	- If we incorrectly detect a text region with confidence we have an even higher focal loss
	- Pushes the model to be especially careful about missing text regions
	- Focal Loss Rank:
		- HIGHEST → Missing text with confidence
		- HIGH    → False text detection with confidence
		- MEDIUM  → Uncertain predictions
		- LOW     → Correct predictions with confidence
#### Recognition
- Use CTC Loss for my recognition model training (Connectionist Temporal Classification), e.g., "Heelloo" and "Helelo" both yield low loss because both collapse to "Hello" CTC doesn't care how we got there. 
- CER 

## Comparison with Open Source Models
#### OCRS by Robert Knight Fine-Tuned (my model)
<img width="781" height="498" alt="Pasted image 20250514101641" src="https://github.com/user-attachments/assets/d0575d60-b0dc-4a07-9563-3c7a5286980d" />


| Raw OCR Output                                                                               | Post-Processed Output                                           |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| 100<br>-104<br>102<br>102<br>112 g<br>S<br>110 <br>108<br>108<br>'112 <br>110 108 Fig. I 106 | 100<br>104<br>102<br>112g<br>110<br>108<br>112<br>FIG. I<br>106 |


![[Pasted image 20250511104523.png]] 

| Raw OCR Output                                                                                                                                                                                                                                      | Post-Processed Output                                                                                                                                                                                                                                                  |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0170<br>104<br>101 14?1.<br>131-1<br>115\. 110<br>1160-1<br>165-2 165-1<br>111/l/ 131-2 5 167-2<br>166-2<br>14)-2 //120<br>142-2 150-3<br>132-2 <br>FIG. 1B<br>142-1 132-1<br>130-1130-2<br>1011402<br>167-1 150-2<br>-150-1<br>113-<br>112<br>114- | 170<br>104<br>101<br>141<br>131-1<br>115<br>110<br>1160-1<br>165-2<br>165-1<br>111<br>131-2<br>5<br>167-2<br>166-2<br>14-2<br>120<br>142-2<br>150-3<br>132-2<br>FIG. 1B<br>142-1<br>132-1<br>130-1<br>130-2<br>1011402<br>167-1<br>150-2<br>150-1<br>113<br>112<br>114 |
#### Tesseract (a popular open source OCR model)
<img width="595" height="429" alt="tesseract" src="https://github.com/user-attachments/assets/58649996-424c-4eb6-a6d1-0f97028d4fd5" />


| Raw OCR Output                          | Post-Processed Output              |
| --------------------------------------- | ---------------------------------- |
| 112 <br>y ~ ~ 10 <br>108 1a gee 81g €d3 | 112<br>10<br>108<br>1a<br>81g<br>3 |


<img width="843" height="932" alt="Pasted image 20250514100912" src="https://github.com/user-attachments/assets/1ac5d754-5a20-4203-8c8b-8c0502a031ef" />


| Raw OCR Output                                                                                                                                                                                                   | Post-Processed Output |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- |
| 100 170<br>- a 21 1304<br>PSS<br>" EEN<br>ha! 150-1<br>131-4 A<br>130-1 \| 130-2<br>18 h 140-1 140-2<br>160-1 \|<br>ke ) 167-1 150-2<br>\~~)<br>— ~—120<br>><br>12<br>14" ~~}<br>142-2 150-3<br>132:2<br>FIG. 1B | TBD                   |

#### Easy OCR (another popular open source OCR model)
<img width="1321" height="928" alt="easyocrbike" src="https://github.com/user-attachments/assets/ab81019b-4367-4951-8e94-3a2d5c437e96" />


| Raw OCR Output                                                                                                    | Post-Processed Output                                             |
| ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 100<br>104<br>102<br>102<br>112<br>4<br>9<br>8<br>9<br>110<br>108<br>108<br>112<br>106<br>Fig:<br>1<br>106<br>110 | 100<br>104<br>102<br>112<br>4<br>9<br>8<br>110<br>108<br>106<br>1 |

<img width="952" height="1156" alt="easyocr2" src="https://github.com/user-attachments/assets/e7fc0762-6b2b-4580-999f-396da99221ed" />


| Raw OCR Output                                                                                                                                                                                                    | Post-Processed Output                                                                                                                                                                                         |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 142-1<br>132-1<br>102<br>50-1<br>131-1<br>130-=<br>130-2<br>140-1=<br>140-2<br>160-1<br>167-1<br>50-2<br>165-2<br>165-1<br>131-2<br>167-2<br>160-2<br>141-2<br>113-<br>1442<br>142-2<br>150-3<br>132-2<br>FIG. 1B | 142-1<br>132-1<br>102<br>50-1<br>131-1<br>130<br>130-2<br>140-1<br>140-2<br>160-1<br>167-1<br>50-2<br>165-2<br>165-1<br>131-2<br>167-2<br>160-2<br>141-2<br>113<br>1442<br>142-2<br>150-3<br>132-2<br>FIG. 1B |

## Performance Metrics

<img width="1979" height="1180" alt="output" src="https://github.com/user-attachments/assets/5709385f-aeec-4e2c-8b62-34dbb22afebb" />
Precision = What fraction of detected text regions are actually text regions?
Recall = How many text regions out of the total text regions in the image did we find?

<img width="1979" height="1180" alt="output (1)" src="https://github.com/user-attachments/assets/9469db6e-433f-4d85-b946-120262a2f9bc" />
Loss = how wrong the model is

<img width="521" height="257" alt="image" src="https://github.com/user-attachments/assets/ffe91b85-eb4c-469f-97e8-35ffd4bc57d7" />
CER = incorrectly identified character/total characters, e.g., "Hello World" vs. "Helo Wrld"

<img width="521" height="310" alt="image" src="https://github.com/user-attachments/assets/b78eae0d-6d5b-48b8-9319-f0caaef2c2da" />
Loss = how wrong the model is
